//! [`Root`] and [`Scope`].

use std::cell::{Cell, RefCell};

use slotmap::{Key, SlotMap};
use smallvec::SmallVec;

use crate::{create_signal, Mark, NodeHandle, NodeId, NodeState, ReactiveNode};

/// The struct managing the state of the reactive system. Only one should be created per running
/// app.
///
/// Often times, this is intended to be leaked to be able to get a `&'static Root`. However, the
/// `Root` is also `dispose`-able, meaning that any resources allocated in this `Root` will get
/// deallocated. Therefore in practice, there should be no memory leak at all except for the `Root`
/// itself. Finally, the `Root` is expected to live for the whole duration of the app so this is
/// not a problem.
pub(crate) struct Root {
    /// If this is `Some`, that means we are tracking signal accesses.
    pub tracker: RefCell<Option<DependencyTracker>>,
    /// A temporary buffer used in `propagate_updates` to prevent allocating a new Vec every time
    /// it is called.
    pub rev_sorted_buf: RefCell<Vec<NodeId>>,
    /// The current node that owns everything created in its scope.
    /// If we are at the top-level, then this is the "null" key.
    pub current_node: Cell<NodeId>,
    /// All the nodes created in this `Root`.
    pub nodes: RefCell<SlotMap<NodeId, ReactiveNode>>,
    /// A list of signals who need their values to be propagated after the batch is over.
    pub node_update_queue: RefCell<Vec<NodeId>>,
    /// Whether we are currently batching signal updatse. If this is true, we do not run
    /// `effect_queue` and instead wait until the end of the batch.
    pub batching: Cell<bool>,
}

thread_local! {
    /// The current reactive root.
    static GLOBAL_ROOT: Cell<Option<&'static Root>> = Cell::new(None);
}

impl Root {
    /// Get the current reactive root. Panics if no root is found.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn global() -> &'static Root {
        GLOBAL_ROOT.with(|root| root.get()).expect("no root found")
    }

    /// Sets the current reactive root. Returns the previous root.
    pub fn set_global(root: Option<&'static Root>) -> Option<&'static Root> {
        GLOBAL_ROOT.with(|r| r.replace(root))
    }

    pub fn new_static() -> &'static Self {
        let this = Self {
            tracker: RefCell::new(None),
            rev_sorted_buf: RefCell::new(Vec::new()),
            current_node: Cell::new(NodeId::null()),
            nodes: RefCell::new(SlotMap::default()),
            node_update_queue: RefCell::new(Vec::new()),
            batching: Cell::new(false),
        };
        let _ref = Box::leak(Box::new(this));
        _ref.reinit();
        _ref
    }

    /// Disposes of all the resources held on by this root and resets the state.
    pub fn reinit(&'static self) {
        // Dispose of all the top-level nodes.
        let top_level = self
            .nodes
            .borrow()
            .iter()
            .filter_map(|(id, value)| {
                if value.parent.is_null() {
                    Some(id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for node in top_level {
            NodeHandle(node, self).dispose();
        }

        let _ = self.tracker.take();
        let _ = self.rev_sorted_buf.take();
        let _ = self.node_update_queue.take();
        let _ = self.current_node.take();
        let _ = self.nodes.take();
        self.batching.set(false);
    }

    /// Create a new child scope. Implementation detail for [`create_child_scope`].
    pub fn create_child_scope(&'static self, f: impl FnOnce()) -> NodeHandle {
        let node = create_signal(()).id;
        let prev = self.current_node.replace(node);
        f();
        self.current_node.set(prev);
        NodeHandle(node, self)
    }

    /// Run the provided closure in a tracked scope. This will detect all the signals that are
    /// accessed and track them in a dependency list.
    pub fn tracked_scope<T>(&self, f: impl FnOnce() -> T) -> (T, DependencyTracker) {
        let prev = self.tracker.replace(Some(DependencyTracker::default()));
        let ret = f();
        (ret, self.tracker.replace(prev).unwrap())
    }

    /// Run the update callback of the signal, also recreating any dependencies found by
    /// tracking signal accesses inside the function.
    ///
    /// Also marks all the dependencies as dirty and marks the current node as clean.
    ///
    /// # Params
    /// * `root` - The reactive root.
    /// * `id` - The id associated with the reactive node.
    /// `SignalId` inside the state itself.
    #[cfg_attr(
        all(feature = "trace", not(debug_assertions)),
        tracing::instrument(skip(self))
    )]
    #[cfg_attr(
        all(feature = "trace", debug_assertions),
        tracing::instrument(skip(self), fields(created_at = self.nodes.borrow()[current].created_at.to_string()))
    )]
    fn run_node_update(&'static self, current: NodeId) {
        debug_assert_eq!(
            self.nodes.borrow()[current].state,
            NodeState::Dirty,
            "should only update when dirty"
        );
        // Remove old dependency links.
        let dependencies = std::mem::take(&mut self.nodes.borrow_mut()[current].dependencies);
        for dependency in dependencies {
            self.nodes.borrow_mut()[dependency]
                .dependents
                .retain(|&id| id != current);
        }
        // We take the callback out because that requires a mut ref and we cannot hold that while
        // running update itself.
        let mut nodes_mut = self.nodes.borrow_mut();
        let mut callback = nodes_mut[current].callback.take().unwrap();
        let mut value = nodes_mut[current].value.take().unwrap();
        drop(nodes_mut); // End RefMut borrow.

        NodeHandle(current, self).dispose_children(); // Destroy anything created in a previous update.

        let prev = self.current_node.replace(current);
        let (changed, tracker) = self.tracked_scope(|| callback(&mut value));
        self.current_node.set(prev);

        tracker.create_dependency_link(self, current);

        let mut nodes_mut = self.nodes.borrow_mut();
        nodes_mut[current].callback = Some(callback); // Put the callback back in.
        nodes_mut[current].value = Some(value);

        // Mark this node as clean.
        nodes_mut[current].state = NodeState::Clean;
        drop(nodes_mut);

        if changed {
            self.mark_dependents_dirty(current);
        }
    }

    // Mark any dependent node of the current node as dirty.
    fn mark_dependents_dirty(&self, current: NodeId) {
        let mut nodes_mut = self.nodes.borrow_mut();
        let dependents = std::mem::take(&mut nodes_mut[current].dependents);
        for &dependent in &dependents {
            if let Some(dependent) = nodes_mut.get_mut(dependent) {
                dependent.state = NodeState::Dirty;
            }
        }
        nodes_mut[current].dependents = dependents;
    }

    /// If there are no cyclic dependencies, then the reactive graph is a DAG (Directed Acylic
    /// Graph). We can therefore use DFS to get a topological sorting of all the reactive nodes.
    ///
    /// We then go through every node in this topological sorting and update only those nodes which
    /// have dependencies that were updated.
    fn propagate_node_updates(&'static self, start_nodes: &[NodeId]) {
        // Try to reuse the shared buffer if possible.
        let mut rev_sorted = Vec::new();
        let mut rev_sorted_buf = self.rev_sorted_buf.try_borrow_mut();
        let rev_sorted = if let Ok(rev_sorted_buf) = rev_sorted_buf.as_mut() {
            rev_sorted_buf.clear();
            rev_sorted_buf
        } else {
            &mut rev_sorted
        };

        // Traverse reactive graph.
        for &node in start_nodes {
            Self::dfs(node, &mut self.nodes.borrow_mut(), rev_sorted);
            self.mark_dependents_dirty(node);
        }
        #[cfg(feature = "trace")]
        tracing::trace!("update len: {}", rev_sorted.len());

        for &node in rev_sorted.iter().rev() {
            let mut nodes_mut = self.nodes.borrow_mut();
            // Only run if node is still alive.
            if nodes_mut.get(node).is_none() {
                continue;
            }
            let node_state = &mut nodes_mut[node];
            node_state.mark = Mark::None; // Reset value.

            // Check if this node needs to be updated.
            if nodes_mut[node].state == NodeState::Dirty {
                drop(nodes_mut); // End RefMut borrow.
                self.run_node_update(node)
            };
        }
    }

    /// Call this if `start_node` has been updated manually. This will automatically update all
    /// signals that depend on `start_node`.
    ///
    /// If we are currently batching, defers updating the signal until the end of the batch.
    #[cfg_attr(debug_assertions, track_caller)]
    #[cfg_attr(
        all(feature = "trace", not(debug_assertions)),
        tracing::instrument(skip(self))
    )]
    #[cfg_attr(
        all(feature = "trace", debug_assertions),
        tracing::instrument(skip(self), fields(created_at = self.nodes.borrow()[start_node].created_at.to_string()))
    )]
    pub fn propagate_updates(&'static self, start_node: NodeId) {
        if self.batching.get() {
            self.node_update_queue.borrow_mut().push(start_node);
        } else {
            // Set the global root.
            let prev = Root::set_global(Some(self));
            // Propagate any signal updates.
            self.propagate_node_updates(&[start_node]);
            Root::set_global(prev);
        }
    }

    /// Run depth-first-search on the reactive graph starting at `current`.
    fn dfs(current_id: NodeId, nodes: &mut SlotMap<NodeId, ReactiveNode>, buf: &mut Vec<NodeId>) {
        let Some(current) = nodes.get_mut(current_id) else {
            // If signal is dead, don't even visit it.
            return;
        };

        match current.mark {
            Mark::Temp => panic!("cyclic reactive dependency"),
            Mark::Permanent => return,
            Mark::None => {}
        }
        current.mark = Mark::Temp;

        // Take the `dependents` field out temporarily to avoid borrow checker.
        let children = std::mem::take(&mut current.dependents);
        for child in &children {
            Self::dfs(*child, nodes, buf);
        }
        nodes[current_id].dependents = children;

        nodes[current_id].mark = Mark::Permanent;
        buf.push(current_id);
    }

    /// Sets the batch flag to `true`.
    fn start_batch(&self) {
        self.batching.set(true);
    }

    /// Sets the batch flag to `false` and run all the queued effects.
    fn end_batch(&'static self) {
        self.batching.set(false);
        let nodes = self.node_update_queue.take();
        self.propagate_node_updates(&nodes);
    }
}

/// A handle to a root. This lets you reinitialize or dispose the root for resource cleanup.
///
/// This is generally obtained from [`create_root`].
#[derive(Clone, Copy)]
pub struct RootHandle {
    _ref: &'static Root,
}

impl RootHandle {
    /// Destroy everything that was created in this scope.
    pub fn dispose(&self) {
        self._ref.reinit();
    }

    /// Runs the closure in the current scope of the root.
    pub fn run_in<T>(&self, f: impl FnOnce() -> T) -> T {
        let prev = Root::set_global(Some(self._ref));
        let ret = f();
        Root::set_global(prev);
        ret
    }
}

/// Tracks nodes that are accessed inside a reactive scope.
#[derive(Default)]
pub(crate) struct DependencyTracker {
    /// A list of reactive nodes that were accessed.
    pub dependencies: SmallVec<[NodeId; 1]>,
}

impl DependencyTracker {
    /// Sets the `dependents` field for all the nodes that have been tracked and updates
    /// `dependencies` of the `dependent`.
    pub fn create_dependency_link(self, root: &Root, dependent: NodeId) {
        for node in &self.dependencies {
            root.nodes.borrow_mut()[*node].dependents.push(dependent);
        }
        // Set the signal dependencies so that it is updated automatically.
        root.nodes.borrow_mut()[dependent].dependencies = self.dependencies;
    }
}

/// Creates a new reactive root with a top-level [`Scope`]. The returned [`RootHandle`] can be used
/// to [`dispose`](RootHandle::dispose) the root.
///
/// # Example
/// ```rust
/// # use sycamore_reactive::*;
///
/// create_root(|| {
///     let signal = create_signal(123);
///
///     let child_scope = create_child_scope(move || {
///         // ...
///     });
/// });
/// ```
#[must_use = "root should be disposed"]
pub fn create_root(f: impl FnOnce()) -> RootHandle {
    let _ref = Root::new_static();
    #[cfg(not(target_arch = "wasm32"))]
    {
        /// An unsafe wrapper around a raw pointer which we promise to never touch, effectively
        /// making it thread-safe.
        #[allow(dead_code)]
        struct UnsafeSendPtr<T>(*const T);
        /// We never ever touch the pointer inside so surely this is safe!
        unsafe impl<T> Send for UnsafeSendPtr<T> {}

        /// A static variable to keep on holding to the allocated `Root`s to prevent Miri and
        /// Valgrind from complaining.
        static KEEP_ALIVE: std::sync::Mutex<Vec<UnsafeSendPtr<Root>>> =
            std::sync::Mutex::new(Vec::new());
        KEEP_ALIVE
            .lock()
            .unwrap()
            .push(UnsafeSendPtr(_ref as *const Root));
    }

    Root::set_global(Some(_ref));
    _ref.create_child_scope(f);
    Root::set_global(None);
    RootHandle { _ref }
}

/// Create a child scope.
///
/// Returns the created [`NodeHandle`] which can be used to dispose it.
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_child_scope(f: impl FnOnce()) -> NodeHandle {
    Root::global().create_child_scope(f)
}

/// Adds a callback that is called when the scope is destroyed.
///
/// # Example
/// ```rust
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let child_scope = create_child_scope(|| {
///     on_cleanup(|| {
///         println!("Child scope is being dropped");
///     });
/// });
/// child_scope.dispose(); // Executes the on_cleanup callback.
/// # });
/// ```
pub fn on_cleanup(f: impl FnOnce() + 'static) {
    let root = Root::global();
    if !root.current_node.get().is_null() {
        root.nodes.borrow_mut()[root.current_node.get()]
            .cleanups
            .push(Box::new(f));
    }
}

/// Batch updates from related signals together and only run memos and effects at the end of the
/// scope.
///
/// # Example
///
/// ```
/// # use sycamore_reactive::*;
/// # let _ = create_root(|| {
/// let state = create_signal(1);
/// let double = create_memo(move || state.get() * 2);
/// batch(move || {
///     state.set(2);
///     assert_eq!(double.get(), 2);
/// });
/// assert_eq!(double.get(), 4);
/// # });
/// ```
pub fn batch<T>(f: impl FnOnce() -> T) -> T {
    let root = Root::global();
    root.start_batch();
    let ret = f();
    root.end_batch();
    ret
}

/// Run the passed closure inside an untracked dependency scope.
///
/// See also [`ReadSignal::get_untracked()`].
///
/// # Example
///
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let state = create_signal(1);
/// let double = create_memo(move || untrack(|| state.get() * 2));
/// assert_eq!(double.get(), 2);
///
/// state.set(2);
/// // double value should still be old value because state was untracked
/// assert_eq!(double.get(), 2);
/// # });
/// ```
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    untrack_in_scope(f, Root::global())
}

/// Same as [`untrack`] but for a specific [`Root`].
pub(crate) fn untrack_in_scope<T>(f: impl FnOnce() -> T, root: &'static Root) -> T {
    let prev = root.tracker.replace(None);
    let ret = f();
    root.tracker.replace(prev);
    ret
}

/// Get a handle to the current reactive scope.
pub fn use_current_scope() -> NodeHandle {
    let root = Root::global();
    NodeHandle(root.current_node.get(), root)
}

/// Get a handle to the root reactive scope.
pub fn use_global_scope() -> NodeHandle {
    let root = Root::global();
    NodeHandle(NodeId::null(), root)
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn cleanup() {
        let _ = create_root(|| {
            let cleanup_called = create_signal(false);
            let scope = create_child_scope(|| {
                on_cleanup(move || {
                    cleanup_called.set(true);
                });
            });
            assert!(!cleanup_called.get());
            scope.dispose();
            assert!(cleanup_called.get());
        });
    }

    #[test]
    fn cleanup_in_effect() {
        let _ = create_root(|| {
            let trigger = create_signal(());

            let counter = create_signal(0);

            create_effect(move || {
                trigger.track();

                on_cleanup(move || {
                    counter.set(counter.get() + 1);
                });
            });

            assert_eq!(counter.get(), 0);

            trigger.set(());
            assert_eq!(counter.get(), 1);

            trigger.set(());
            assert_eq!(counter.get(), 2);
        });
    }

    #[test]
    fn cleanup_is_untracked() {
        let _ = create_root(|| {
            let trigger = create_signal(());

            let counter = create_signal(0);

            create_effect(move || {
                counter.set(counter.get_untracked() + 1);

                on_cleanup(move || {
                    trigger.track(); // trigger should not be tracked
                });
            });

            assert_eq!(counter.get(), 1);

            trigger.set(());
            assert_eq!(counter.get(), 1);
        });
    }

    #[test]
    fn batch_memo() {
        let _ = create_root(|| {
            let state = create_signal(1);
            let double = create_memo(move || state.get() * 2);
            batch(move || {
                state.set(2);
                assert_eq!(double.get(), 2);
            });
            assert_eq!(double.get(), 4);
        });
    }

    #[test]
    fn batch_updates_effects_at_end() {
        let _ = create_root(|| {
            let state1 = create_signal(1);
            let state2 = create_signal(2);
            let counter = create_signal(0);
            create_effect(move || {
                counter.set(counter.get_untracked() + 1);
                let _ = state1.get() + state2.get();
            });
            assert_eq!(counter.get(), 1);
            state1.set(2);
            state2.set(3);
            assert_eq!(counter.get(), 3);
            batch(move || {
                state1.set(3);
                assert_eq!(counter.get(), 3);
                state2.set(4);
                assert_eq!(counter.get(), 3);
            });
            assert_eq!(counter.get(), 4);
        });
    }
}
