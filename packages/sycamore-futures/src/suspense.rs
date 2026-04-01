//! Suspense with first class `async`/`await` support.
//!
//! The [`Suspense`] component is used to "suspend" execution and wait until async tasks are
//! finished before rendering.

use futures::channel::oneshot;
use futures::Future;
use sycamore_reactive::*;

use crate::*;

/// A context value that keeps track of all the signals representing the number of tasks remaining
/// in a suspense scope.
///
/// This is useful for figuring out when all suspense tasks are completed on the page.
#[derive(Copy, Clone, Debug, Default)]
struct AllTasksRemaining {
    all_tasks_remaining: Signal<Vec<Signal<u32>>>,
}

/// Represents a new suspense scope. This is created by a call to [`create_suspense_scope`].
#[derive(Copy, Clone, Debug)]
pub struct SuspenseScope {
    tasks_remaining: Signal<u32>,
    /// The parent suspense scope of the current scope, if it exists.
    pub parent: Option<Signal<SuspenseScope>>,
    /// Signal that is set to `true` when the view is rendered and streamed into the buffer.
    /// This is unused on the client side.
    pub sent: Signal<bool>,
}

impl SuspenseScope {
    /// Create a new suspense scope, optionally with a parent scope.
    ///
    /// The parent scope should always be located in a reactive scope that is an ancestor of
    /// this scope.
    pub fn new(parent: Option<SuspenseScope>) -> Self {
        let tasks_remaining = create_signal(0);
        let global = use_global_scope().run_in(|| use_context_or_else(AllTasksRemaining::default));
        global
            .all_tasks_remaining
            .update(|vec| vec.push(tasks_remaining));
        // TODO: remove self from global if scope is disposed.
        Self {
            tasks_remaining,
            parent: parent.map(create_signal),
            sent: create_signal(false),
        }
    }

    /// Implementation for [`Self::is_loading`]. This is used to recursively check whether we are
    /// loading or not.
    fn _is_loading(self) -> bool {
        self.tasks_remaining.get() > 0
            || self
                .parent
                .as_ref()
                .is_some_and(|parent| parent.get()._is_loading())
    }

    /// Returns a signal representing whether we are currently loading this suspense or not.
    ///
    /// Implementation for the [`use_is_loading`] hook.
    pub fn is_loading(self) -> ReadSignal<bool> {
        create_selector(move || self._is_loading())
    }

    /// Returns a future that resolves once the scope is no longer loading.
    pub async fn until_finished(self) {
        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);
        create_effect(move || {
            if !self._is_loading() {
                if let Some(tx) = tx.take() {
                    tx.send(()).unwrap();
                }
            }
        });

        rx.await.unwrap()
    }
}

/// A guard that keeps a suspense scope suspended until it is dropped.
#[derive(Debug)]
pub struct SuspenseTaskGuard {
    scope: Option<SuspenseScope>,
}

impl SuspenseTaskGuard {
    /// Creates a new suspense task guard. This will suspend the current suspense scope until this
    /// guard is dropped.
    pub fn new() -> Self {
        let scope = try_use_context::<SuspenseScope>();
        if let Some(mut scope) = scope {
            scope.tasks_remaining += 1;
        }
        Self { scope }
    }

    /// Create a new suspense task guard from a suspense scope.
    pub fn from_scope(mut scope: SuspenseScope) -> Self {
        scope.tasks_remaining += 1;
        Self { scope: Some(scope) }
    }
}

impl Default for SuspenseTaskGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SuspenseTaskGuard {
    fn drop(&mut self) {
        if let Some(mut scope) = self.scope {
            // It's possible that the scope has already been destroyed, in the case where the
            // suspense task is still running after the suspense scope is destroyed. In this case,
            // we need to be careful not to access any values in the scope since that will cause a
            // panic.
            if scope.tasks_remaining.is_alive() {
                scope.tasks_remaining -= 1;
            }
        }
    }
}

/// Creates a new task that is to be tracked by the suspense system.
///
/// This is used to signal to a `Suspense` component higher up in the component hierarchy that
/// there is some async task that should be awaited before showing the UI.
///
/// If this is called from outside a suspense scope, the task will be executed normally.
pub fn create_suspense_task(f: impl Future<Output = ()> + 'static) {
    let guard = SuspenseTaskGuard::new();
    spawn_local_scoped(async move {
        f.await;
        drop(guard);
    });
}

/// Create a new suspense scope that is detached from the rest of the suspense hierarchy.
///
/// This is useful if you want the result of this suspense to be independent of the parent suspense
/// scope.
///
/// It is rarely recommended to use this function as it can lead to unexpected behavior when using
/// server side rendering, and in particular, streaming. Instead, use [`create_suspense_scope`].
///
/// The reason for this is because we generally expect outer suspenses to be resolved first before
/// an inner suspense is resolved, since otherwise we would have no place to show the inner suspense
/// as the outer fallback is still being displayed.
pub fn create_detached_suspense_scope<T>(f: impl FnOnce() -> T) -> (T, SuspenseScope) {
    let scope = SuspenseScope::new(None);
    provide_context_in_new_scope(scope, move || {
        let ret = f();
        (ret, scope)
    })
}

// TODO: remove this in the next major version
#[allow(missing_docs)]
#[deprecated = "Please use `create_detached_suspense_scope` instead"]
pub fn create_detatched_suspense_scope<T>(f: impl FnOnce() -> T) -> (T, SuspenseScope) {
    create_detached_suspense_scope(f)
}

/// Calls the given function and registers all suspense tasks.
///
/// Returns a tuple containing the return value of the function and the created suspense scope.
///
/// If this is called inside another call to [`create_suspense_scope`], this suspense will wait
/// until the parent suspense is resolved.
pub fn create_suspense_scope<T>(f: impl FnOnce() -> T) -> (T, SuspenseScope) {
    let parent = try_use_context::<SuspenseScope>();
    let scope = SuspenseScope::new(parent);
    provide_context_in_new_scope(scope, move || {
        let ret = f();
        (ret, scope)
    })
}

/// Waits until all suspense task in current scope are completed.
///
/// Does not create a new suspense scope.
///
/// If not called inside a suspense scope, the future will resolve immediately.
pub async fn await_suspense_current() {
    if let Some(scope) = try_use_context::<SuspenseScope>() {
        scope.until_finished().await;
    }
}

/// Returns a signal representing whether we are currently loading this suspense or not.
///
/// This will be true if there are any tasks remaining in this scope or in any parent
/// scope.
///
/// This function is also reactive and so the loading state can be tracked. If it is called outside
/// of a suspense scope, the signal will always be `false`.
pub fn use_is_loading() -> ReadSignal<bool> {
    try_use_context::<SuspenseScope>().map_or(*create_signal(false), |scope| scope.is_loading())
}

/// Returns whether any suspense scope is current loading.
///
/// This is unlike [`use_is_loading`] in that it can be called outside of a suspense scope and does
/// not apply to any suspense scope in particular.
pub fn use_is_loading_global() -> bool {
    if let Some(global) = try_use_context::<AllTasksRemaining>() {
        global
            .all_tasks_remaining
            .with(|vec| vec.iter().any(|signal| signal.get() > 0))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn suspense_scope() {
        let _ = create_root(|| {
            let _ = create_suspense_scope(|| {
                let outer_scope = try_use_context::<SuspenseScope>();
                assert!(outer_scope.is_some());
                assert!(outer_scope.unwrap().parent.is_none());

                let _ = create_suspense_scope(|| {
                    let inner_scope = try_use_context::<SuspenseScope>();
                    assert!(inner_scope.is_some());
                    assert!(inner_scope.unwrap().parent.is_some());
                });
            });
        });
    }

    #[tokio::test]
    async fn suspense_await_suspense() {
        let (tx, rx) = oneshot::channel();
        let is_completed = Rc::new(Cell::new(false));

        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let _ = create_root({
                    let is_completed = is_completed.clone();
                    || {
                        spawn_local_scoped(async move {
                            let (_, scope) = create_suspense_scope(|| {
                                create_suspense_task(async move {
                                    rx.await.unwrap();
                                });
                            });

                            scope.until_finished().await;
                            is_completed.set(true);
                        });
                    }
                });
            })
            .await;

        assert!(!is_completed.get());

        tx.send(()).unwrap();
        local.await;
        assert!(is_completed.get());
    }

    #[tokio::test]
    async fn use_is_loading_global_works() {
        let (tx, rx) = oneshot::channel();

        let local = tokio::task::LocalSet::new();
        let mut root = create_root(|| {});
        local
            .run_until(async {
                root = create_root(|| {
                    let _ = create_suspense_scope(|| {
                        create_suspense_task(async move {
                            rx.await.unwrap();
                        });
                    });
                });
            })
            .await;

        root.run_in(|| {
            assert!(use_is_loading_global());
        });

        tx.send(()).unwrap();
        local.await;

        root.run_in(|| {
            assert!(!use_is_loading_global());
        });
    }

    /// Create a suspense task that never completes, and then destroy the suspense scope. This
    /// should not cause any panics or memory leaks.
    #[test]
    fn suspense_task_completing_after_suspense_scope_destroyed() {
        // HACK: Currently, Tokio will swallow panics in spawned tasks unless the handle is
        // awaited. Since we do not await the handle of the task created by `create_suspense_task`,
        // we set a custom panic hook that will abort the test if any task panics.
        std::panic::set_hook(Box::new(|info| {
            eprintln!("Task panicked: {info}");
            panic!("aborting test due to panic in task");
        }));

        let (_tx, rx) = oneshot::channel::<()>();

        let local = tokio::task::LocalSet::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        local.block_on(&rt, async {
            let _root = create_root(|| {
                let trigger = create_signal(());
                let rx = create_signal(Some(rx));
                create_effect(move || {
                    trigger.track();
                    let _ = create_suspense_scope(move || {
                        create_suspense_task(async move {
                            if let Some(rx) = rx.take() {
                                rx.await.unwrap();
                            }
                        });
                    });
                });
                trigger.set(());
            });
        });
    }
}
