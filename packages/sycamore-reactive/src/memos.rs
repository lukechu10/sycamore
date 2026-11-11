//! Memos (aka. eager derived signals).

use std::cell::RefCell;

use crate::{MutSignal, Root, create_empty_signal, create_signal};

/// Creates a memoized value from some signals.
/// Unlike [`create_memo`], this function will not notify dependents of a
/// change if the output is the same.
///
/// It takes a comparison function to compare the old and new value, which returns `true` if
/// they are the same and `false` otherwise.
///
/// To use the type's [`PartialEq`] implementation instead of a custom function, use
/// [`create_selector`].
///
/// Like [`create_memo`], the returned signal can be written to directly. The comparison function
/// is used only during recomputation and compares the new result with the current stored value,
/// including any manual writes. Direct writes use ordinary [`MutSignal`] notification behavior.
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_selector_with<T>(
    mut f: impl FnMut() -> T + 'static,
    mut eq: impl FnMut(&T, &T) -> bool + 'static,
) -> MutSignal<T> {
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

    signal
}

/// Creates a memoized computation from some signals.
///
/// The output is derived from all the signals that are used within the memo closure.
/// If any of the tracked signals are updated, the memo is also updated.
///
/// The returned signal can also be written to directly. Writes notify dependents without changing
/// the source signals. The next dependency-triggered recomputation may overwrite the manually
/// written value, including when a dependency is updated in the same [`crate::batch`].
/// Dereference the returned signal to obtain a read-only [`Signal`](crate::Signal):
///
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let source = create_signal(2);
/// let memo = create_memo(move || source.get() * 2);
/// memo.set(100);
/// assert_eq!(memo.get(), 100);
/// assert_eq!(source.get(), 2);
/// source.set(3);
/// assert_eq!(memo.get(), 6);
/// let readonly: Signal<i32> = *memo;
/// # });
/// ```
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
pub fn create_memo<T>(f: impl FnMut() -> T + 'static) -> MutSignal<T> {
    create_selector_with(f, |_, _| false)
}

/// Creates a memoized value from some signals.
///
/// Unlike [`create_memo`], this function will not notify dependents of a change if the output is
/// the same. That is why the output of the function must implement [`PartialEq`].
///
/// To specify a custom comparison function, use [`create_selector_with`].
///
/// Like [`create_memo`], the returned signal can be written to directly. During recomputation,
/// the new result is compared with the current stored value, including any manual writes.
/// Direct writes use ordinary [`MutSignal`] notification behavior and do not check equality.
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
pub fn create_selector<T>(f: impl FnMut() -> T + 'static) -> MutSignal<T>
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
/// Returns a writable [`MutSignal`] and a dispatch function to send messages to the reducer.
/// Direct writes notify dependents without calling the reducer. Subsequent dispatches use the
/// current value, including any direct writes. To expose only read access and require mutations
/// through dispatch, dereference the returned signal to obtain a [`Signal`](crate::Signal).
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
) -> (MutSignal<T>, impl Fn(Msg)) {
    let reduce = RefCell::new(reduce);
    let signal = create_signal(initial);
    let dispatch = move |msg| signal.update(|value| *value = reduce.borrow_mut()(value, msg));
    (signal, dispatch)
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

    #[test]
    fn writable_memos() {
        let constructors: [fn(MutSignal<i32>) -> MutSignal<i32>; 4] = [
            |source| create_memo(move || source.get() * 2),
            |source| create_selector(move || source.get() * 2),
            |source| create_selector_with(move || source.get() * 2, PartialEq::eq),
            |source| source.map(|value| value * 2),
        ];
        for constructor in constructors {
            let _ = create_root(|| {
                let source = create_signal(2);
                let memo = constructor(source);
                let readonly: Signal<i32> = *memo;
                let observed = create_signal(0);
                create_effect(move || observed.set(readonly.get()));

                memo.set(100);
                assert_eq!(readonly.get(), 100);
                assert_eq!(observed.get(), 100);
                assert_eq!(source.get(), 2);

                memo.update(|value| *value += 1);
                assert_eq!(observed.get(), 101);

                source.set(3);
                assert_eq!(readonly.get(), 6);
                assert_eq!(observed.get(), 6);

                // Recomputing a dirty memo at the end of a batch overwrites a manual write,
                // regardless of which write came first.
                for source_first in [true, false] {
                    batch(|| {
                        if source_first {
                            source.set(4);
                            memo.set(200);
                        } else {
                            memo.set(200);
                            source.set(4);
                        }
                    });
                    assert_eq!(readonly.get(), 8);
                    assert_eq!(observed.get(), 8);
                }
            });
        }
    }

    #[test]
    fn writable_selector_comparison() {
        let _ = create_root(|| {
            let source = create_signal(2);
            let selector = create_selector_with(move || source.get(), |a, b| a % 2 == b % 2);
            let calls = create_signal(0);
            create_effect(move || {
                selector.track();
                calls.update(|value| *value += 1);
            });

            // Direct writes bypass the comparator, even when the value is unchanged.
            selector.set(2);
            assert_eq!(calls.get(), 2);
            selector.set(5);
            assert_eq!(calls.get(), 3);

            // Compare with the override (5), not the previous computed value (2).
            source.set(3);
            assert_eq!(selector.get(), 5);
            assert_eq!(calls.get(), 3);
            source.set(4);
            assert_eq!(selector.get(), 4);
            assert_eq!(calls.get(), 4);
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
    fn writable_reducer() {
        let _ = create_root(|| {
            let reducer_calls = create_signal(0);
            let (state, dispatch) = create_reducer(0, move |state, delta: i32| {
                reducer_calls.update(|calls| *calls += 1);
                state + delta
            });
            let readonly: Signal<i32> = *state;
            let observed = create_signal(0);
            create_effect(move || observed.set(readonly.get()));

            state.set(10);
            assert_eq!(readonly.get(), 10);
            assert_eq!(observed.get(), 10);
            assert_eq!(reducer_calls.get(), 0);

            dispatch(2);
            assert_eq!(readonly.get(), 12);
            assert_eq!(observed.get(), 12);
            assert_eq!(reducer_calls.get(), 1);

            state.update(|value| *value *= 2);
            assert_eq!(observed.get(), 24);
            assert_eq!(reducer_calls.get(), 1);

            dispatch(-1);
            assert_eq!(readonly.get(), 23);
            assert_eq!(observed.get(), 23);
            assert_eq!(reducer_calls.get(), 2);
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
