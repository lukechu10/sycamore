//! Utilities for components and component properties.

use sycamore_reactive::*;

/// Runs the given closure inside a new component scope. In particular, it does the following:
/// * Create a new trapping tracked scope (see [`trapping_scope`]).
#[doc(hidden)]
pub fn component_scope<T>(f: impl FnOnce() -> T) -> T {
    trapping_scope(f)
}
