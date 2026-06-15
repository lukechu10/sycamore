//! Memos (aka. eager derived signals).

use std::cell::RefCell;

use crate::{ReadSignal, Root, create_empty_signal, create_signal};

/// Creates a memoized value from some signals.
/// Unlike [`create_memo`], this function will not notify dependents of a
/// change if the output is the same.
///
/// It takes a comparison function to compare the old and new value, which returns `true` if
/// they are the same and `false` otherwise.
///
/// To use the type's [`PartialEq`] implementation instead of a custom function, use
/// [`create_selector`].
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_selector_with<T>(
    mut f: impl FnMut() -> T + 'static,
    mut eq: impl FnMut(&T, &T) -> bool + 'static,
) -> ReadSignal<T> {
    let root = Root::global();
    let signal = create_empty_signal();
    let prev = root.current_node.replace(signal.id);
    let (initial, tracker) = root.tracked_scope(&mut f);
    root.current_node.set(prev);

    tracker.create_dependency_link(root, signal.id);

    let mut signal_mut = signal.get_mut();
    signal_mut.value = Some(Box::new(initial));
    signal_mut.callback = Some(Box::new(move |value| {
        let value = value.downcast_mut().expect("wrong memo type");
        let new = f();
        if eq(&new, value) {
            false
        } else {
            *value = new;
            true
        }
    }));

    *signal
}

/// Creates a memoized computation from some signals.
///
/// The output is derived from all the signals that are used within the memo closure.
/// If any of the tracked signals are updated, the memo is also updated.
///
/// # Difference from derived signals
///
/// Derived signals (functions referencing signals) are lazy and do not keep track of the result
/// of the computation. This means that the computation will not be executed until needed.
/// This also means that calling the derived signal twice will result in the same computation
/// twice.
///
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let state = create_signal(0);
/// let double = || state.get() * 2;
///
/// let _ = double();
/// // Here, the closure named double is called again.
/// // If the computation is expensive enough, this would be wasted work!
/// let _ = double();
/// # });
/// ```
///
/// Memos, on the other hand, are eagerly evaluated and will only run the computation when one
/// of its dependencies change.
///
/// Memos also incur a slightly higher performance penalty than simple derived signals, so unless
/// there is some computation involved, it will likely be faster to just use a derived signal.
///
/// # Example
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let state = create_signal(0);
/// let double = create_memo(move || state.get() * 2);
///
/// assert_eq!(double.get(), 0);
/// state.set(1);
/// assert_eq!(double.get(), 2);
/// # });
/// ```
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_memo<T>(f: impl FnMut() -> T + 'static) -> ReadSignal<T> {
    create_selector_with(f, |_, _| false)
}

/// Creates a memoized value from some signals.
///
/// Unlike [`create_memo`], this function will not notify dependents of a change if the output is
/// the same. That is why the output of the function must implement [`PartialEq`].
///
/// To specify a custom comparison function, use [`create_selector_with`].
///
/// # Example
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let state = create_signal(1);
/// let squared = create_selector(move || state.get() * state.get());
/// assert_eq!(squared.get(), 1);
///
/// create_effect(move || println!("x^2 = {}", squared.get()));
///
/// state.set(2); // Triggers the effect.
/// assert_eq!(squared.get(), 4);
///
/// state.set(-2); // Does not trigger the effect.
/// assert_eq!(squared.get(), 4);
/// # });
/// ```
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_selector<T>(f: impl FnMut() -> T + 'static) -> ReadSignal<T>
where
    T: PartialEq,
{
    create_selector_with(f, PartialEq::eq)
}

/// An alternative to [`create_signal`] that uses a reducer to get the next
/// value.
///
/// It uses a reducer function that takes the previous value and a message and returns the next
/// value.
///
/// Returns a [`ReadSignal`] and a dispatch function to send messages to the reducer.
///
/// # Params
/// * `initial` - The initial value of the state.
/// * `reducer` - A function that takes the previous value and a message and returns the next value.
///
/// # Example
/// ```
/// # use sycamore_reactive::*;
/// enum Msg {
///     Increment,
///     Decrement,
/// }
///
/// # create_root(|| {
/// let (state, dispatch) = create_reducer(0, |&state, msg: Msg| match msg {
///     Msg::Increment => state + 1,
///     Msg::Decrement => state - 1,
/// });
///
/// assert_eq!(state.get(), 0);
/// dispatch(Msg::Increment);
/// assert_eq!(state.get(), 1);
/// dispatch(Msg::Decrement);
/// assert_eq!(state.get(), 0);
/// # });
/// ```
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_reducer<T, Msg>(
    initial: T,
    reduce: impl FnMut(&T, Msg) -> T,
) -> (ReadSignal<T>, impl Fn(Msg)) {
    let reduce = RefCell::new(reduce);
    let signal = create_signal(initial);
    let dispatch = move |msg| signal.update(|value| *value = reduce.borrow_mut()(value, msg));
    (*signal, dispatch)
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn memo() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let double = create_memo(move || state.get() * 2);

            assert_eq!(double.get(), 0);
            state.set(1);
            assert_eq!(double.get(), 2);
            state.set(2);
            assert_eq!(double.get(), 4);
        });
    }

    /// Make sure value is memoized rather than executed on demand.
    #[test]
    fn memo_only_run_once() {
        let _ = create_root(|| {
            let state = create_signal(0);

            let counter = create_signal(0);
            let double = create_memo(move || {
                counter.set_silent(counter.get_untracked() + 1);
                state.get() * 2
            });

            assert_eq!(counter.get(), 1); // once for calculating initial derived state
            state.set(2);
            assert_eq!(counter.get(), 2);
            assert_eq!(double.get(), 4);
            assert_eq!(counter.get(), 2); // should still be 2 after access
        });
    }

    #[test]
    fn dependency_on_memo() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let double = create_memo(move || state.get() * 2);
            let quadruple = create_memo(move || double.get() * 2);

            assert_eq!(quadruple.get(), 0);
            state.set(1);
            assert_eq!(quadruple.get(), 4);
        });
    }

    #[test]
    fn untracked_memo() {
        let _ = create_root(|| {
            let state = create_signal(1);
            let double = create_memo(move || state.get_untracked() * 2);

            assert_eq!(double.get(), 2);
            state.set(2);
            assert_eq!(double.get(), 2); // double value should still be true because state.get()
            // was
            // inside untracked
        });
    }

    #[test]
    fn memos_should_recreate_dependencies_each_time() {
        let _ = create_root(|| {
            let condition = create_signal(true);

            let state1 = create_signal(0);
            let state2 = create_signal(1);

            let counter = create_signal(0);
            create_memo(move || {
                counter.set_silent(counter.get_untracked() + 1);

                if condition.get() {
                    state1.track();
                } else {
                    state2.track();
                }
            });

            assert_eq!(counter.get(), 1);

            state1.set(1);
            assert_eq!(counter.get(), 2);

            state2.set(1);
            assert_eq!(counter.get(), 2); // not tracked

            condition.set(false);
            assert_eq!(counter.get(), 3);

            state1.set(2);
            assert_eq!(counter.get(), 3); // not tracked

            state2.set(2);
            assert_eq!(counter.get(), 4); // tracked after condition.set
        });
    }

    #[test]
    fn destroy_memos_on_scope_dispose() {
        let _ = create_root(|| {
            let counter = create_signal(0);

            let trigger = create_signal(());

            let child_scope = create_child_scope(move || {
                let _ = create_memo(move || {
                    trigger.track();
                    counter.set_silent(counter.get_untracked() + 1);
                });
            });

            assert_eq!(counter.get(), 1);

            trigger.set(());
            assert_eq!(counter.get(), 2);

            child_scope.dispose();
            trigger.set(());
            assert_eq!(counter.get(), 2); // memo should be destroyed and thus not executed
        });
    }

    #[test]
    fn selector() {
        let _ = create_root(|| {
            let state = create_signal(0);
            let double = create_selector(move || state.get() * 2);

            let counter = create_signal(0);
            create_effect(move || {
                counter.set(counter.get_untracked() + 1);

                double.track();
            });
            assert_eq!(double.get(), 0);
            assert_eq!(counter.get(), 1);

            state.set(0);
            state.set(0);
            state.set(0);
            assert_eq!(double.get(), 0);
            assert_eq!(counter.get(), 1);

            state.set(2);
            assert_eq!(double.get(), 4);
            assert_eq!(counter.get(), 2);
        });
    }

    #[test]
    fn reducer() {
        let _ = create_root(|| {
            enum Msg {
                Increment,
                Decrement,
            }

            let (state, dispatch) = create_reducer(0, |state, msg: Msg| match msg {
                Msg::Increment => *state + 1,
                Msg::Decrement => *state - 1,
            });

            assert_eq!(state.get(), 0);
            dispatch(Msg::Increment);
            assert_eq!(state.get(), 1);
            dispatch(Msg::Decrement);
            assert_eq!(state.get(), 0);
            dispatch(Msg::Increment);
            dispatch(Msg::Increment);
            assert_eq!(state.get(), 2);
        });
    }

    #[test]
    fn memo_reducer() {
        let _ = create_root(|| {
            enum Msg {
                Increment,
                Decrement,
            }

            let (state, dispatch) = create_reducer(0, |state, msg: Msg| match msg {
                Msg::Increment => *state + 1,
                Msg::Decrement => *state - 1,
            });
            let doubled = create_memo(move || state.get() * 2);

            assert_eq!(doubled.get(), 0);
            dispatch(Msg::Increment);
            assert_eq!(doubled.get(), 2);
            dispatch(Msg::Decrement);
            assert_eq!(doubled.get(), 0);
        });
    }
}
