//! Reactive primitives for [Sycamore](https://github.com/sycamore-rs/sycamore).
//!
//! ```rust
//! use sycamore_reactive::*;
//!
//! create_root(|| {
//!     let greeting = create_signal("Hello");
//!     let name = create_signal("World");
//!
//!     let display_text = create_memo(move || format!("{greeting} {name}!"));
//!     assert_eq!(display_text.get_clone(), "Hello World!");
//!
//!     name.set("Sycamore");
//!     assert_eq!(display_text.get_clone(), "Hello Sycamore!");
//! });
//! ```
//!
//! # Strict mode
//!
//! By default, a signal that is accessed outside of a tracking scope will emit an warning. This is
//! to prevent cases where a signal should be accessed reactively but is not, leading to bugs.
//!
//! Strict mode changes this behavior to panic instead of warning. This can be useful for testing
//! and debugging, as it will immediately alert you to any reactive access issues. Enable strict
//! mode by adding the `strict` feature to `sycamore-reactive` or to `sycamore`.
//!
//! # A note on `nightly`
//!
//! If you are using rust `nightly`, you can enable the `nightly` feature to enable the more terse
//! syntax for signal get/set.
//!
//! ```ignore
//! # use sycamore_reactive::*;
//! # create_root(|| {
//! let signal = create_signal(123);
//!
//! // Stable:
//! let value = signal.get();
//! signal.set(456);
//!
//! // Nightly:
//! let value = signal();
//! signal(456);
//! # });
//! ```
//! Of course, the stable `.get()` also works on nightly as well if that's what you prefer.

#![warn(missing_docs)]
#![cfg_attr(feature = "nightly", feature(fn_traits, unboxed_closures))]

mod context;
mod effects;
mod iter;
mod maybe_dyn;
mod memos;
mod node;
mod root;
mod signals;
mod utils;

pub use context::*;
pub use effects::*;
pub use iter::*;
pub use maybe_dyn::*;
pub use memos::*;
pub use node::*;
pub use root::*;
pub use signals::*;
pub use utils::*;

/// Add name for proc-macro purposes.
extern crate self as sycamore_reactive;
