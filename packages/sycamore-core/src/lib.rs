//! Core functionality for the Sycamore UI framework.
//!
//! This crate should not be used directly. Instead, use the `sycamore` crate which re-exports this
//! crate.

mod component;
#[doc(hidden)]
pub mod props_runtime;

pub use component::*;
