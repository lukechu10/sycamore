//! Macro support for the `sycamore-router` crate.

#![deny(missing_debug_implementations)]
#![warn(missing_docs)]

mod parser;
mod route;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// The `Route` procedural macro.
///
/// This macro derives the `Route` trait for the given `enum`.
#[proc_macro_derive(Route, attributes(to, not_found))]
pub fn route(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    route::route_impl(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
