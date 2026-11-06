//! Core functionality for the Sycamore UI framework.
//!
//! This crate should not be used directly. Instead, use the `sycamore` crate which re-exports this
//! crate.

mod component;
#[doc(hidden)]
pub mod props_runtime;

pub use component::*;

/// Builds a value of type `T` from a builder.
///
/// Most commonly, the generic `T` is a `View` type and the trait is implemented for all types that
/// can be converted into a `View`. This would include things like `&str`, `String`, `View` itself,
/// and in particular, component builders.
pub trait Build<T> {
    /// Build the value of type `T`.
    fn build(self) -> T;
}

/// A trait for `View` types that can be constructed from a function.
///
/// It is generally expected that the resulting `View` will be dynamic and will update when the
/// function's return value changes.
pub trait BuildFromFn {
    fn from_fn(f: impl FnMut() -> Self + 'static) -> Self;
}

impl<T: BuildFromFn, F: FnMut() -> Output + 'static, Output: Build<T> + 'static> Build<T> for F {
    fn build(mut self) -> T {
        T::from_fn(move || self().build())
    }
}
