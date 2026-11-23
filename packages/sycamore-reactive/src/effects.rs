//! Side effects!

use crate::create_memo;
use crate::memos::create_computed_node;

/// Creates an effect on signals used inside the effect closure.
///
/// # Example
/// ```
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let state = create_signal(0);
///
/// create_effect(move || {
///     println!("new state = {}", state.get());
/// });
/// // Prints "new state = 0"
///
/// state.set(1);
/// // Prints "new state = 1"
/// # });
/// ```
///
/// `create_effect` should only be used for creating **side-effects**. It is generally not
/// recommended to update signal states inside an effect. You probably should be using a
/// [`create_memo`](crate::create_memo) instead.
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_effect(f: impl FnMut() + 'static) {
    create_memo(f);
}

/// Creates an effect that runs a different code path on the first run.
///
/// The initial function runs immediately and returns a tuple containing a function for subsequent
/// runs and a value returned to the caller. The initializer can borrow local data; only the
/// function for subsequent runs must be `'static`.
///
/// # Example
/// ```
/// # use sycamore_reactive::*;
/// let label = String::from("count");
/// # create_root(|| {
/// let state = create_signal(0);
///
/// let initial_label = create_effect_initial(|| {
///     println!("{label}: {}", state.get()); // Track `state` on the first run.
///     (Box::new(move || println!("new count: {}", state.get())), label.as_str())
/// });
/// assert_eq!(initial_label, "count");
/// state.set(1);
/// # });
/// ```
///
/// The rerun callback cannot borrow local data:
///
/// ```compile_fail,E0597
/// # use sycamore_reactive::*;
/// # create_root(|| {
/// let label = String::from("count");
/// create_effect_initial(|| (Box::new(|| println!("{label}")), ()));
/// # });
/// ```
///
/// Note that the initial function is also called within the effect scope. This means that signals
/// created within the initial function will no longer be alive in subsequent runs. If you want to
/// create signals that are alive in subsequent runs, you should use
/// [`use_current_scope`](crate::use_current_scope) and
/// [`NodeHandle::run_in`](crate::NodeHandle::run_in).
#[cfg_attr(debug_assertions, track_caller)]
pub fn create_effect_initial<T>(initial: impl FnOnce() -> (Box<dyn FnMut() + 'static>, T)) -> T {
    let (_, result) = create_computed_node(|| {
        let (mut rerun, result) = initial();
        let update = move |_: &mut ()| {
            rerun();
            true
        };
        ((), update, result)
    });
    result
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn effect() {
        let _ = create_root(|| {
            let state = create_signal(0);

            let double = create_signal(-1);

            create_effect(move || {
                double.set(state.get() * 2);
            });
            assert_eq!(double.get(), 0); // calling create_effect should call the effect at least once

            state.set(1);
            assert_eq!(double.get(), 2);
            state.set(2);
            assert_eq!(double.get(), 4);
        });
    }

    #[test]
    fn effect_initial_runs_once_and_replaces_dependencies() {
        let _ = create_root(|| {
            let initial_source = create_signal(0);
            let rerun_source = create_signal(0);
            let mut initial_runs = 0;
            let reruns = create_signal(0);

            let result = create_effect_initial(|| {
                initial_runs += 1;
                let value = initial_source.get();
                // This write must not trigger a rerun before initialization finishes.
                initial_source.set(value + 1);
                (
                    Box::new(move || {
                        rerun_source.track();
                        reruns.set(reruns.get_untracked() + 1);
                    }),
                    value,
                )
            });
            assert_eq!(result, 0);
            assert_eq!(initial_runs, 1);
            assert_eq!(reruns.get(), 0);

            // The rerun-only source is not tracked until the callback first runs.
            rerun_source.set(1);
            assert_eq!(reruns.get(), 0);
            initial_source.set(2);
            assert_eq!(reruns.get(), 1);
            initial_source.set(3);
            assert_eq!(reruns.get(), 1);
            rerun_source.set(2);
            assert_eq!(reruns.get(), 2);
        });
    }

    #[test]
    fn effect_initial_cleans_up_before_rerun() {
        let _ = create_root(|| {
            let source = create_signal(0);
            let cleaned_up = create_signal(false);
            let reran = create_signal(false);
            let child = create_effect_initial(|| {
                source.track();
                let child = create_signal(());
                on_cleanup(move || cleaned_up.set(true));
                (
                    Box::new(move || {
                        assert!(cleaned_up.get_untracked());
                        assert!(!child.is_alive());
                        reran.set(true);
                    }),
                    child,
                )
            });
            assert!(child.is_alive());
            assert!(!cleaned_up.get());
            assert!(!reran.get());

            source.set(1);
            assert!(reran.get());
        });
    }

    #[test]
    fn effect_with_explicit_dependencies() {
        let _ = create_root(|| {
            let state = create_signal(0);

            let double = create_signal(-1);

            create_effect(on(state, move || {
                double.set(state.get() * 2);
            }));
            assert_eq!(double.get(), 0); // calling create_effect should call the effect at least once

            state.set(1);
            assert_eq!(double.get(), 2);
            state.set(2);
            assert_eq!(double.get(), 4);
        });
    }

    #[test]
    fn effect_cannot_create_infinite_loop() {
        let _ = create_root(|| {
            let state = create_signal(0);
            create_effect(move || {
                state.track();
                state.set(0);
            });
            state.set(0);
        });
    }

    #[test]
    fn effect_should_only_subscribe_once_to_same_signal() {
        let _ = create_root(|| {
            let state = create_signal(0);

            let counter = create_signal(0);
            create_effect(move || {
                counter.set(counter.get_untracked() + 1);

                // call state.track() twice but should subscribe once
                state.track();
                state.track();
            });

            assert_eq!(counter.get(), 1);

            state.set(1);
            assert_eq!(counter.get(), 2);
        });
    }

    #[test]
    fn effect_should_recreate_dependencies_each_time() {
        let _ = create_root(|| {
            let condition = create_signal(true);

            let state1 = create_signal(0);
            let state2 = create_signal(1);

            let counter = create_signal(0);
            create_effect(move || {
                counter.set(counter.get_untracked() + 1);

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
    fn outer_effects_run_first() {
        let _ = create_root(|| {
            let trigger = create_signal(());

            let outer_counter = create_signal(0);
            let inner_counter = create_signal(0);

            create_effect(move || {
                trigger.track();
                outer_counter.set(outer_counter.get_untracked() + 1);

                create_effect(move || {
                    trigger.track();
                    inner_counter.set(inner_counter.get_untracked() + 1);
                });
            });

            assert_eq!(outer_counter.get(), 1);
            assert_eq!(inner_counter.get(), 1);

            trigger.set(());

            assert_eq!(outer_counter.get(), 2);
            assert_eq!(inner_counter.get(), 2);
        });
    }

    /// Regression test for https://github.com/sycamore-rs/sycamore/issues/794
    ///
    /// For details, see this comment: https://github.com/sycamore-rs/sycamore/issues/794#issuecomment-4166087556
    #[test]
    fn nested_effects_rerun_inner() {
        let _ = create_root(|| {
            let state = create_signal(1);
            let double = create_memo(move || state.get() * 2);
            let is_even = create_memo(move || state.get() % 2 == 0);

            let last_updated = create_signal(double.get());
            create_effect(move || {
                if is_even.get() {
                    create_effect(move || {
                        last_updated.set(double.get());
                    });
                }
            });

            // Initial run.
            assert_eq!(last_updated.get(), 2);
            // Inner effect should be created and access clean value for `double`.
            state.set(2);
            assert_eq!(last_updated.get(), 4);
        });
    }

    /// Regression test for https://github.com/sycamore-rs/sycamore/issues/794
    ///
    /// For details, see this comment: https://github.com/sycamore-rs/sycamore/issues/794#issuecomment-4166087556
    #[test]
    fn effect_uses_new_value_for_new_dependency() {
        let _ = create_root(|| {
            let state = create_signal(1);
            let double = create_memo(move || state.get() * 2);
            let is_even = create_memo(move || state.get() % 2 == 0);

            let last_updated = create_signal(double.get());
            create_effect(move || {
                if is_even.get() {
                    last_updated.set(double.get());
                }
            });

            // Initial run.
            assert_eq!(last_updated.get(), 2);
            // Accessing `double` after a dependency change uses the clean/new value.
            state.set(2);
            assert_eq!(last_updated.get(), 4);
        });
    }

    #[test]
    fn destroy_effects_on_scope_dispose() {
        let _ = create_root(|| {
            let counter = create_signal(0);

            let trigger = create_signal(());

            let child_scope = create_child_scope(move || {
                create_effect(move || {
                    trigger.track();
                    counter.set(counter.get_untracked() + 1);
                });
            });

            assert_eq!(counter.get(), 1);

            trigger.set(());
            assert_eq!(counter.get(), 2);

            child_scope.dispose();
            trigger.set(());
            assert_eq!(counter.get(), 2); // inner effect should be destroyed and thus not executed
        });
    }

    #[test]
    fn effect_scoped_subscribing_to_own_signal() {
        let _ = create_root(|| {
            let trigger = create_signal(());
            create_effect(move || {
                trigger.track();
                let signal = create_signal(());
                // Track own signal:
                signal.track();
            });
            trigger.set(());
        });
    }
}
