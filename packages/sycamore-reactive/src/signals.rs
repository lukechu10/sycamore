//! Reactive signals.

use std::cell::{Ref, RefMut};
use std::fmt;
use std::fmt::Formatter;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{AddAssign, Deref, DivAssign, MulAssign, RemAssign, SubAssign};

use slotmap::Key;
use smallvec::SmallVec;

use crate::*;

/// A read-only reactive value.
///
/// Unlike the difference between Rust's shared and mutable-references (`&T` and `&mut`), the
/// underlying data is not immutable. The data can be updated with the corresponding [`Signal`]
/// (which has mutable access) and will show up in the `ReadSignal` as well.
///
/// A `ReadSignal` can be simply obtained by dereferencing a [`Signal`]. In fact, every [`Signal`]
/// is a `ReadSignal` with additional write abilities!
///
/// # Example
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let signal: Signal<i32> = create_signal(123);
/// let read_signal: ReadSignal<i32> = *signal;
/// assert_eq!(read_signal.get(), 123);
/// signal.set(456);
/// assert_eq!(read_signal.get(), 456);
/// // read_signal.set(789); // <-- This is not allowed!
/// # });
/// ```
///
/// See [`create_signal`] for more information.
pub struct ReadSignal<T: 'static> {
    pub(crate) id: NodeId,
    root: &'static Root,
    /// Keep track of where the signal was created for diagnostics.
    /// This is also stored in the Node but we want to have access to this when accessing a
    /// disposed node so we store it here as well.
    #[cfg(debug_assertions)]
    created_at: &'static std::panic::Location<'static>,
    _phantom: PhantomData<T>,
}

/// A reactive value that can be read and written to.
///
/// This is the writable version of [`ReadSignal`].
///
/// See [`create_signal`] for more information.
pub struct Signal<T: 'static>(pub(crate) ReadSignal<T>);

/// Create a new [`Signal`].
///
/// Signals are reactive atoms, pieces of state that can be read and written to and which will
/// automatically update anything which depend on them.
///
/// # Usage
/// The simplest way to use a signal is by using [`.get()`](ReadSignal::get) and
/// [`.set(...)`](Signal::set). However, this only works if the value implements [`Copy`]. If
/// we wanted to store something that doesn't implement [`Copy`] but implements [`Clone`] instead,
/// say a [`String`], we can use [`.get_clone()`](ReadSignal::get_clone) which will automatically
/// clone the value for us.
///
/// ```rust
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let signal = create_signal(1);
/// signal.get(); // Should return 1.
/// signal.set(2);
/// signal.get(); // Should return 2.
/// # });
/// ```
///
/// There are many other ways of getting and setting signals, such as
/// [`.with(...)`](ReadSignal::with) and [`.update(...)`](Signal::update) which can access the
/// signal even if it does not implement [`Clone`] or if you simply don't want to pay the
/// performance overhead of cloning your value every time you read it.
///
/// # Reactivity
/// What makes signals so powerful, as opposed to some other wrapper type like
/// [`RefCell`](std::cell::RefCell) is the automatic dependency tracking. This means that accessing
/// a signal will automatically add it as a dependency in certain contexts (such as inside a
/// [`create_memo`](crate::create_memo)) which allows us to update related state whenever the signal
/// is changed.
///
/// ```rust
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let signal = create_signal(1);
/// // Note that we are accessing signal inside a closure in the line below. This will cause it to
/// // be automatically tracked and update our double value whenever signal is changed.
/// let double = create_memo(move || signal.get() * 2);
/// double.get(); // Should return 2.
/// signal.set(2);
/// double.get(); // Should return 4. Notice how this value was updated automatically when we
///               // modified signal. This way, we can rest assured that all our state will be
///               // consistent at all times!
/// # });
/// ```
///
/// # Ownership
/// Signals are always associated with a reactive node. This is what performs the memory management
/// for the actual value of the signal. What is returned from this function is just a
/// handle/reference to the signal allocated in the reactive node. This allows us to freely copy
/// this handle around and use it in closures and event handlers without worrying about ownership of
/// the signal.
///
/// This is why in the above example, we could access `signal` even after it was moved in to the
/// closure of the `create_memo`.
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_signal<T>(value: T) -> Signal<T> {
    let signal = create_empty_signal();
    signal.get_mut().value = Some(Box::new(value));
    signal
}

/// Creates a new [`Signal`] with the `value` field set to `None`.
#[cfg_attr(debug_assertions, track_caller)]
pub(crate) fn create_empty_signal<T>() -> Signal<T> {
    let root = Root::global();
    let id = root.nodes.borrow_mut().insert(ReactiveNode {
        value: None,
        callback: None,
        children: Vec::new(),
        parent: root.current_node.get(),
        dependents: Vec::new(),
        dependencies: SmallVec::new(),
        cleanups: Vec::new(),
        context: Vec::new(),
        state: NodeState::Clean,
        mark: Mark::None,
        #[cfg(debug_assertions)]
        created_at: std::panic::Location::caller(),
    });
    // Add the signal to the parent's `children` list.
    let current_node = root.current_node.get();
    if !current_node.is_null() {
        root.nodes.borrow_mut()[current_node].children.push(id);
    }

    Signal(ReadSignal {
        id,
        root,
        #[cfg(debug_assertions)]
        created_at: std::panic::Location::caller(),
        _phantom: PhantomData,
    })
}

impl<T> ReadSignal<T> {
    /// Get a immutable reference to the underlying node.
    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn get_ref(self) -> Ref<'static, ReactiveNode> {
        Ref::map(
            self.root
                .nodes
                .try_borrow()
                .expect("cannot read signal while updating"),
            |nodes| match nodes.get(self.id) {
                Some(node) => node,
                None => panic!("{}", self.get_disposed_panic_message()),
            },
        )
    }

    /// Get a mutable reference to the underlying node.
    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn get_mut(self) -> RefMut<'static, ReactiveNode> {
        RefMut::map(
            self.root
                .nodes
                .try_borrow_mut()
                .expect("cannot update signal while reading"),
            |nodes| match nodes.get_mut(self.id) {
                Some(node) => node,
                None => panic!("{}", self.get_disposed_panic_message()),
            },
        )
    }

    /// Returns `true` if the signal is still alive, i.e. has not yet been disposed.
    pub fn is_alive(self) -> bool {
        self.root.nodes.borrow().get(self.id).is_some()
    }

    /// Disposes the signal, i.e. frees up the memory held on by this signal. Accessing a signal
    /// after it has been disposed immediately causes a panic.
    pub fn dispose(self) {
        NodeHandle(self.id, self.root).dispose();
    }

    fn get_disposed_panic_message(self) -> String {
        #[cfg(not(debug_assertions))]
        return "signal was disposed".to_string();

        #[cfg(debug_assertions)]
        return format!("signal was disposed. Created at {}", self.created_at);
    }

    /// Get the value of the signal without tracking it. The type must implement [`Copy`]. If this
    /// is not the case, use [`ReadSignal::get_clone_untracked`] or [`ReadSignal::with_untracked`]
    /// instead.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(0);
    /// // Note that we have used `get_untracked` here so the signal is not actually being tracked
    /// // by the memo.
    /// let doubled = create_memo(move || state.get_untracked() * 2);
    /// state.set(1);
    /// assert_eq!(doubled.get(), 0);
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_untracked(self) -> T
    where
        T: Copy,
    {
        self.with_untracked(|value| *value)
    }

    /// Get the value of the signal without tracking it. The type is [`Clone`]-ed automatically.
    ///
    /// This is the cloned equivalent of [`ReadSignal::get_untracked`].
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_clone_untracked(self) -> T
    where
        T: Clone,
    {
        self.with_untracked(Clone::clone)
    }

    /// Get the value of the signal. The type must implement [`Copy`]. If this is not the case, use
    /// [`ReadSignal::get_clone_untracked`] or [`ReadSignal::with_untracked`] instead.
    ///
    /// When called inside a reactive scope, the signal will be automatically tracked.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(0);
    /// assert_eq!(state.get(), 0);
    ///
    /// state.set(1);
    /// assert_eq!(state.get(), 1);
    ///
    /// // The signal is automatically tracked in the line below.
    /// let doubled = create_memo(move || state.get());
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get(self) -> T
    where
        T: Copy,
    {
        self.track();
        self.get_untracked()
    }

    /// Get the value of the signal. The type is [`Clone`]-ed automatically.
    ///
    /// When called inside a reactive scope, the signal will be automatically tracked.
    ///
    /// If the value implements [`Copy`], you should use [`ReadSignal::get`] instead.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let greeting = create_signal("Hello".to_string());
    /// assert_eq!(greeting.get_clone(), "Hello".to_string());
    ///
    /// // The signal is automatically tracked in the line below.
    /// let hello_world = create_memo(move || format!("{} World!", greeting.get_clone()));
    /// assert_eq!(hello_world.get_clone(), "Hello World!");
    ///
    /// greeting.set("Goodbye".to_string());
    /// assert_eq!(greeting.get_clone(), "Goodbye".to_string());
    /// assert_eq!(hello_world.get_clone(), "Goodbye World!");
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_clone(self) -> T
    where
        T: Clone,
    {
        self.track();
        self.get_clone_untracked()
    }

    /// Get a value from the signal without tracking it.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn with_untracked<U>(self, f: impl FnOnce(&T) -> U) -> U {
        self.root.ensure_node_is_clean(self.id);

        let node = self.get_ref();
        debug_assert!(
            node.state == NodeState::Clean,
            "state should always be latest value"
        );
        let value = node
            .value
            .as_ref()
            .expect("cannot read signal while updating");
        let ret = f(value.downcast_ref().expect("wrong signal type"));
        ret
    }

    /// Get a value from the signal.
    ///
    /// When called inside a reactive scope, the signal will be automatically tracked.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn with<U>(self, f: impl FnOnce(&T) -> U) -> U {
        self.track();
        self.with_untracked(f)
    }

    /// Creates a new [memo](create_memo) from this signal and a function. The resulting memo will
    /// be created in the current reactive scope.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(0);
    /// let doubled = state.map(|val| *val * 2);
    /// assert_eq!(doubled.get(), 0);
    /// state.set(1);
    /// assert_eq!(doubled.get(), 2);
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn map<U>(self, mut f: impl FnMut(&T) -> U + 'static) -> ReadSignal<U> {
        create_memo(move || self.with(&mut f))
    }

    /// Track the signal in the current reactive scope. This is done automatically when calling
    /// [`ReadSignal::get`] and other similar methods.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(0);
    /// create_effect(move || {
    ///     state.track(); // Track the signal without getting its value.
    ///     println!("Yipee!");
    /// });
    /// state.set(1); // Prints "Yipee!"
    /// # });
    /// ```
    pub fn track(self) {
        if let Some(tracker) = &mut *self.root.tracker.borrow_mut() {
            tracker.dependencies.push(self.id);
        }
    }
}

impl<T> Signal<T> {
    /// Silently set a new value for the signal. This will not trigger any updates in dependent
    /// signals. As such, this is generally not recommended as it can easily lead to state
    /// inconsistencies.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(0);
    /// let doubled = create_memo(move || state.get() * 2);
    /// assert_eq!(doubled.get(), 0);
    /// state.set_silent(1);
    /// assert_eq!(doubled.get(), 0); // We now have inconsistent state!
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn set_silent(self, new: T) {
        self.replace_silent(new);
    }

    /// Set a new value for the signal and automatically update any dependents.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(0);
    /// let doubled = create_memo(move || state.get() * 2);
    /// assert_eq!(doubled.get(), 0);
    /// state.set(1);
    /// assert_eq!(doubled.get(), 2);
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn set(self, new: T) {
        self.replace(new);
    }

    /// Silently set a new value for the signal and return the previous value.
    ///
    /// This is the silent version of [`Signal::replace`].
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn replace_silent(self, new: T) -> T {
        self.update_silent(|val| std::mem::replace(val, new))
    }

    /// Set a new value for the signal and return the previous value.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(123);
    /// let prev = state.replace(456);
    /// assert_eq!(state.get(), 456);
    /// assert_eq!(prev, 123);
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn replace(self, new: T) -> T {
        self.update(|val| std::mem::replace(val, new))
    }

    /// Silently gets the value of the signal and sets the new value to the default value.
    ///
    /// This is the silent version of [`Signal::take`].
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn take_silent(self) -> T
    where
        T: Default,
    {
        self.replace_silent(T::default())
    }

    /// Gets the value of the signal and sets the new value to the default value.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(Some(123));
    /// let prev = state.take();
    /// assert_eq!(state.get(), None);
    /// assert_eq!(prev, Some(123));
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn take(self) -> T
    where
        T: Default,
    {
        self.replace(T::default())
    }

    /// Update the value of the signal silently. This will not trigger any updates in dependent
    /// signals. As such, this is generally not recommended as it can easily lead to state
    /// inconsistencies.
    ///
    /// This is the silent version of [`Signal::update`].
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn update_silent<U>(self, f: impl FnOnce(&mut T) -> U) -> U {
        let mut value = self
            .get_mut()
            .value
            .take()
            .expect("cannot update signal while reading");
        let ret = f(value.downcast_mut().expect("wrong signal type"));
        self.get_mut().value = Some(value);
        ret
    }

    /// Update the value of the signal and automatically update any dependents.
    ///
    /// Using this has the advantage of not needing to clone the value when updating it, especially
    /// with types that do not implement `Copy` where cloning can be expensive, or for types that
    /// do not implement `Clone` at all.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal("Hello".to_string());
    /// state.update(|val| val.push_str(" Sycamore!"));
    /// assert_eq!(state.get_clone(), "Hello Sycamore!");
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn update<U>(self, f: impl FnOnce(&mut T) -> U) -> U {
        let ret = self.update_silent(f);
        self.0.root.propagate_updates(self.0.id);
        ret
    }

    /// Use a function to produce a new value and sets the value silently.
    ///
    /// This is the silent version of [`Signal::set_fn`].
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn set_fn_silent(self, f: impl FnOnce(&T) -> T) {
        self.update_silent(move |val| *val = f(val));
    }

    /// Use a function to produce a new value and sets the value.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let state = create_signal(123);
    /// state.set_fn(|val| *val + 1);
    /// assert_eq!(state.get(), 124);
    /// # });
    /// ```
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn set_fn(self, f: impl FnOnce(&T) -> T) {
        self.update(move |val| *val = f(val));
    }

    /// Split the signal into a reader/writer pair.
    ///
    /// # Example
    /// ```
    /// # use sycamore_reactive::*;
    /// # create_root(|| {
    /// let (read_signal, mut write_signal) = create_signal(0).split();
    /// assert_eq!(read_signal.get(), 0);
    /// write_signal(1);
    /// assert_eq!(read_signal.get(), 1);
    /// # });
    /// ```
    pub fn split(self) -> (ReadSignal<T>, impl Fn(T) -> T) {
        (*self, move |value| self.replace(value))
    }
}

/// We manually implement `Clone` + `Copy` for `Signal` so that we don't get extra bounds on `T`.
impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ReadSignal<T> {}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

// Implement `Default` for `ReadSignal` and `Signal`.
impl<T: Default> Default for ReadSignal<T> {
    fn default() -> Self {
        *create_signal(Default::default())
    }
}
impl<T: Default> Default for Signal<T> {
    fn default() -> Self {
        create_signal(Default::default())
    }
}

// Forward `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` from inner type.
impl<T: PartialEq> PartialEq for ReadSignal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.with(|value| other.with(|other| value == other))
    }
}
impl<T: Eq> Eq for ReadSignal<T> {}
impl<T: PartialOrd> PartialOrd for ReadSignal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.with(|value| other.with(|other| value.partial_cmp(other)))
    }
}
impl<T: Ord> Ord for ReadSignal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.with(|value| other.with(|other| value.cmp(other)))
    }
}
impl<T: Hash> Hash for ReadSignal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.with(|value| value.hash(state))
    }
}

impl<T: PartialEq> PartialEq for Signal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn eq(&self, other: &Self) -> bool {
        self.with(|value| other.with(|other| value == other))
    }
}
impl<T: Eq> Eq for Signal<T> {}
impl<T: PartialOrd> PartialOrd for Signal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.with(|value| other.with(|other| value.partial_cmp(other)))
    }
}
impl<T: Ord> Ord for Signal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.with(|value| other.with(|other| value.cmp(other)))
    }
}
impl<T: Hash> Hash for Signal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.with(|value| value.hash(state))
    }
}

impl<T> Deref for Signal<T> {
    type Target = ReadSignal<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Formatting implementations for `ReadSignal` and `Signal`.
impl<T: fmt::Debug> fmt::Debug for ReadSignal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.with(|value| value.fmt(f))
    }
}
impl<T: fmt::Debug> fmt::Debug for Signal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.with(|value| value.fmt(f))
    }
}

impl<T: fmt::Display> fmt::Display for ReadSignal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.with(|value| value.fmt(f))
    }
}
impl<T: fmt::Display> fmt::Display for Signal<T> {
    #[cfg_attr(debug_assertions, track_caller)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.with(|value| value.fmt(f))
    }
}

// Serde implementations for `ReadSignal` and `Signal`.
#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for ReadSignal<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.with(|value| value.serialize(serializer))
    }
}
#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for ReadSignal<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(*create_signal(T::deserialize(deserializer)?))
    }
}
#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for Signal<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.with(|value| value.serialize(serializer))
    }
}
#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Signal<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(create_signal(T::deserialize(deserializer)?))
    }
}

#[cfg(feature = "nightly")]
impl<T: Copy> FnOnce<()> for ReadSignal<T> {
    type Output = T;

    extern "rust-call" fn call_once(self, _args: ()) -> Self::Output {
        self.get()
    }
}

impl<T: AddAssign<Rhs>, Rhs> AddAssign<Rhs> for Signal<T> {
    fn add_assign(&mut self, rhs: Rhs) {
        self.update(|this| *this += rhs);
    }
}
impl<T: SubAssign<Rhs>, Rhs> SubAssign<Rhs> for Signal<T> {
    fn sub_assign(&mut self, rhs: Rhs) {
        self.update(|this| *this -= rhs);
    }
}
impl<T: MulAssign<Rhs>, Rhs> MulAssign<Rhs> for Signal<T> {
    fn mul_assign(&mut self, rhs: Rhs) {
        self.update(|this| *this *= rhs);
    }
}
impl<T: DivAssign<Rhs>, Rhs> DivAssign<Rhs> for Signal<T> {
    fn div_assign(&mut self, rhs: Rhs) {
        self.update(|this| *this /= rhs);
    }
}
impl<T: RemAssign<Rhs>, Rhs> RemAssign<Rhs> for Signal<T> {
    fn rem_assign(&mut self, rhs: Rhs) {
        self.update(|this| *this %= rhs);
    }
}

// We need to implement this again for `Signal` despite `Signal` deref-ing to `ReadSignal` since
// we also have another implementation of `FnOnce` for `Signal`.
#[cfg(feature = "nightly")]
impl<T: Copy> FnOnce<()> for Signal<T> {
    type Output = T;

    extern "rust-call" fn call_once(self, _args: ()) -> Self::Output {
        self.get()
    }
}

#[cfg(feature = "nightly")]
impl<T: Copy> FnOnce<(T,)> for Signal<T> {
    type Output = T;

    extern "rust-call" fn call_once(self, (val,): (T,)) -> Self::Output {
        self.replace(val)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn signal() {
        let _ = create_root(|| {
            let state = create_signal(0);
            assert_eq!(state.get(), 0);

            state.set(1);
            assert_eq!(state.get(), 1);

            state.set_fn(|n| *n + 1);
            assert_eq!(state.get(), 2);
        });
    }

    #[test]
    fn signal_composition() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let double = || state.get() * 2;

            assert_eq!(double(), 0);
            state.set(1);
            assert_eq!(double(), 2);
        });
    }

    #[test]
    fn set_silent_signal() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let double = state.map(|&x| x * 2);

            assert_eq!(double.get(), 0);
            state.set_silent(1);
            assert_eq!(double.get(), 0); // double value is unchanged.

            state.set_fn_silent(|n| n + 1);
            assert_eq!(double.get(), 0); // double value is unchanged.
        });
    }

    #[test]
    fn read_signal() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let readonly: ReadSignal<i32> = *state;

            assert_eq!(readonly.get(), 0);
            state.set(1);
            assert_eq!(readonly.get(), 1);
        });
    }

    #[test]
    fn map_signal() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let double = state.map(|&x| x * 2);

            assert_eq!(double.get(), 0);
            state.set(1);
            assert_eq!(double.get(), 2);
        });
    }

    #[test]
    fn take_signal() {
        let _ = create_root(|| {
            let state = create_signal(123);

            let x = state.take();
            assert_eq!(x, 123);
            assert_eq!(state.get(), 0);
        });
    }

    #[test]
    fn take_silent_signal() {
        let _ = create_root(|| {
            let state = create_signal(123);
            let double = state.map(|&x| x * 2);

            // Do not trigger subscribers.
            state.take_silent();
            assert_eq!(state.get(), 0);
            assert_eq!(double.get(), 246);
        });
    }

    #[test]
    fn signal_split() {
        let _ = create_root(|| {
            let (state, set_state) = create_signal(0).split();
            assert_eq!(state.get(), 0);

            set_state(1);
            assert_eq!(state.get(), 1);
        });
    }

    #[test]
    fn signal_display() {
        let _ = create_root(|| {
            let signal = create_signal(0);
            assert_eq!(format!("{signal}"), "0");
            let read_signal: ReadSignal<_> = *signal;
            assert_eq!(format!("{read_signal}"), "0");
            let memo = create_memo(|| 0);
            assert_eq!(format!("{memo}"), "0");
        });
    }

    #[test]
    fn signal_debug() {
        let _ = create_root(|| {
            let signal = create_signal(0);
            assert_eq!(format!("{signal:?}"), "0");
            let read_signal: ReadSignal<_> = *signal;
            assert_eq!(format!("{read_signal:?}"), "0");
            let memo = create_memo(|| 0);
            assert_eq!(format!("{memo:?}"), "0");
        });
    }

    #[test]
    fn signal_add_assign_update() {
        let _ = create_root(|| {
            let mut signal = create_signal(0);
            let counter = create_signal(0);
            create_effect(move || {
                signal.track();
                counter.set(counter.get_untracked() + 1);
            });
            signal += 1;
            signal -= 1;
            signal *= 1;
            signal /= 1;
            assert_eq!(counter.get(), 5);
        });
    }

    #[test]
    fn signal_update() {
        let _ = create_root(|| {
            let signal = create_signal("Hello ".to_string());
            let counter = create_signal(0);
            create_effect(move || {
                signal.track();
                counter.set(counter.get_untracked() + 1);
            });
            signal.update(|value| value.push_str("World!"));
            assert_eq!(signal.get_clone(), "Hello World!");
            assert_eq!(counter.get(), 2);
        });
    }
}
