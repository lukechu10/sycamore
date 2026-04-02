//! # Sycamore API Documentation
//!
//! Sycamore is a **reactive** library for creating web apps in **Rust** and **WebAssembly**.
//!
//! This is the API docs for sycamore. If you are looking for the usage docs, check out the
//! [Sycamore Book](https://sycamore.dev/book/introduction).
//!
//! ## Feature Flags
//!
//! - `hydrate` - Enables hydration support in DOM nodes. By default, hydration is disabled to
//!   reduce binary size.
//!
//! - `serde` - Enables serializing and deserializing `Signal`s and other wrapper types using
//!   `serde`.
//!
//! - `suspense` - Enables suspense and resources. Also enables wrappers around
//!   `wasm-bindgen-futures` to make it easier to extend a reactive scope into an `async` function.
//!
//! - `nightly` - Enables nightly-only features. This makes it slightly more ergonomic to use
//!   signals.
//!
//! - `wasm-bindgen-interning` (_default_) - Enables interning for `wasm-bindgen` strings. This
//!   improves performance at a slight cost in binary size. If you want to minimize the size of the
//!   result `.wasm` binary, you might want to disable this.
//!
//! - `web` (_default_) - Enables the web backend for Sycamore. This feature is enabled by most of
//!   the other features so you should rarely need to enable it manually.

#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![deny(clippy::trait_duplication_in_bounds)]
#![deny(clippy::type_repetition_in_bounds)]
#![deny(missing_debug_implementations)]

// Alias self to `sycamore` to make it possible to use proc-macros within the `sycamore` crate.
#[allow(unused_extern_crates)] // False positive
extern crate self as sycamore;

pub mod easing;
pub mod motion;

/* Re-export of the sycamore-macro crate */
pub use sycamore_macro::*;

/// Reactive primitives for Sycamore.
///
/// Re-export of the [`sycamore_reactive`] crate.
pub mod reactive {
    pub use sycamore_reactive::*;
}

/// Web support for Sycamore.
///
/// Re-export of the [`sycamore_web`] crate.
pub mod web {
    pub use sycamore_web::*;
}

/// Utilities for working with async.
///
/// Re-export of the [`sycamore_futures`] crate.
#[cfg(feature = "suspense")]
pub mod futures {
    pub use sycamore_futures::*;
}

#[cfg(feature = "hydrate")]
pub use sycamore_web::{hydrate, hydrate_in_scope, hydrate_to};
pub use sycamore_web::{
    render, render_in_scope, render_to, render_to_string, render_to_string_in_scope,
};
#[cfg(feature = "suspense")]
pub use sycamore_web::{render_to_string_await_suspense, render_to_string_stream};

/// The Sycamore prelude.
///
/// In most cases, it is idiomatic to use a glob import (aka wildcard import) at the beginning of
/// your Rust source file.
///
/// ```rust
/// use sycamore::prelude::*;
/// ```
pub mod prelude {
    pub use sycamore_core::{Component, Props};
    #[cfg(feature = "web")]
    pub use sycamore_macro::*;
    #[cfg(feature = "web")]
    pub use sycamore_web::tags::html_attributes::*;
    #[cfg(feature = "web")]
    pub use sycamore_web::tags::svg_attributes::*;
    #[cfg(feature = "web")]
    pub use sycamore_web::{
        console_dbg, console_log, create_node_ref, document, is_not_ssr, is_ssr, on_mount, window,
        Attributes, Children, GlobalAttributes, GlobalProps, HtmlGlobalAttributes, Indexed, Keyed,
        NodeRef, SvgGlobalAttributes, View,
    };

    pub use crate::reactive::*;
}

/// Re-exports for use by `sycamore-macro`. Not intended for use by end-users.
#[doc(hidden)]
pub mod rt {
    pub use sycamore_core::{component_scope, element_like_component_builder, Component, Props};
    #[cfg(feature = "suspense")]
    pub use sycamore_futures::*;
    pub use sycamore_macro::*;
    pub use sycamore_reactive::*;
    #[cfg(feature = "web")]
    pub use sycamore_web::*;
    #[cfg(feature = "web")]
    pub use web_sys::Event;
}
