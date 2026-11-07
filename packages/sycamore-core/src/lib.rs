//! Core functionality for the Sycamore UI framework.
//!
//! This crate should not be used directly. Instead, use the `sycamore` crate which re-exports this
//! crate.

#[doc(hidden)]
pub mod builder_runtime;
mod component;

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

/// Represents a callback that can be called with a value of type `T`.
///
/// The benefit of using this type over a raw `impl FnMut()` or a `Box<dyn FnMut()` is that it
/// implements `Default` and `From` traits which makes it easier to use as component props.
///
/// This type also implements `Deref` to the inner `FnMut` type, so you can call it directly as if
/// it were a function.
pub struct Callback<T, Out = ()>(Box<dyn FnMut(T) -> Out>);

impl<F: FnMut(T) -> Out + 'static, T, Out> From<F> for Callback<T, Out> {
    fn from(f: F) -> Self {
        Self(Box::new(f))
    }
}

impl<T, Out: Default> Default for Callback<T, Out> {
    fn default() -> Self {
        Self(Box::new(|_| Default::default()))
    }
}

impl<T, Out> std::ops::Deref for Callback<T, Out> {
    type Target = dyn FnMut(T) -> Out;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<T, Out> std::ops::DerefMut for Callback<T, Out> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}
