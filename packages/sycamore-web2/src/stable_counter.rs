//! Provides a counter that is stable across client and server side. Useful for generating IDs for
//! accessibility among other things.
//!
//! This works internally by using the context API to store the current value of the counter.

use sycamore_reactive::{Signal, use_context_or_else, use_global_scope};

#[derive(Debug, Default, Clone, Copy)]
struct CounterValue {
    next: Signal<u32>,
}

/// Get the next counter value. This is stable across client and server side.
///
/// The counter is stored in the global scope so that it is shared across the entire app. It is
/// initialized to 0 and incremented every time this function is called.
pub fn use_stable_counter() -> u32 {
    let global_scope = use_global_scope();
    let counter = global_scope.run_in(|| use_context_or_else(CounterValue::default));

    let next = counter.next.get();
    counter.next.set(next + 1);
    next
}

#[cfg(test)]
mod tests {
    use sycamore_reactive::*;

    use super::*;

    #[test]
    fn stable_counter_works() {
        let _ = create_root(|| {
            let counter = use_stable_counter();
            assert_eq!(counter, 0);

            let counter = use_stable_counter();
            assert_eq!(counter, 1);

            let _ = create_child_scope(|| {
                let counter = use_stable_counter();
                assert_eq!(counter, 2);
            });

            let counter = use_stable_counter();
            assert_eq!(counter, 3);
        });
    }
}
