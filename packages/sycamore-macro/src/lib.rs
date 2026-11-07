//! Proc-macros used in [Sycamore](https://docs.rs/sycamore).

#![deny(missing_debug_implementations)]
#![warn(missing_docs)]

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod builder;
mod component;

/// A macro for ergonomically creating complex UI complex layouts.
///
/// To learn more about the view syntax, see [the chapter on the view
/// macro](https://sycamore.dev/book/guide/view-dsl) in the Sycamore Book.
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let root = parse_macro_input!(input as sycamore_view_parser::ir::Root);

    sycamore_view_parser::codegen::Codegen {}.root(&root).into()
}

/// A macro for creating components from functions.
///
/// Add this attribute to a `fn` to create a component from that function.
///
/// To learn more about components, see the chapter on
/// [components](https://sycamore.dev/book/introduction/your-first-app#using-components) in the Sycamore Book.
#[proc_macro_attribute]
pub fn component(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as component::ComponentArgs);

    component::component_impl(args, item.clone().into())
        .unwrap_or_else(|err| {
            // If proc-macro errors, emit the original function for better IDE support.
            let error_tokens = err.into_compile_error();
            let body_input = proc_macro2::TokenStream::from(item);
            quote! {
                #body_input
                #error_tokens
            }
        })
        .into()
}

/// Replaces a function with a zero-argument fluent builder constructor.
#[proc_macro_attribute]
pub fn builder(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "`#[builder]` does not accept arguments",
        )
        .into_compile_error()
        .into();
    }
    let function = parse_macro_input!(input as syn::ItemFn);
    builder::expand(function)
        .map(|expansion| expansion.tokens)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// A macro for feature gating code that should only be run on the server.
///
/// By default, the target is used to determine the rendering mode. However, `--cfg
/// sycamore_force_ssr` can be used to override this behavior.
///
/// # Example
///
/// ```
/// # use sycamore_macro::*;
/// #[cfg_ssr]
/// fn server_only() {}
/// ```
///
/// See also [`macro@cfg_not_ssr`].
#[proc_macro_attribute]
pub fn cfg_ssr(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input: proc_macro2::TokenStream = input.into();
    quote! {
        ::sycamore::web::is_ssr! {
            #input
        }
    }
    .into()
}

/// A macro for feature gating code that should only be run on the web.
///
/// By default, the target is used to determine the rendering mode. However, `--cfg
/// sycamore_force_ssr` can be used to override this behavior.
///
/// # Example
///
/// ```
/// # use sycamore_macro::*;
/// #[cfg_not_ssr]
/// fn browser_only() {}
/// ```
///
/// See also [`macro@cfg_ssr`].
#[proc_macro_attribute]
pub fn cfg_not_ssr(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input: proc_macro2::TokenStream = input.into();
    quote! {
        ::sycamore::web::is_not_ssr! {
            #input
        }
    }
    .into()
}
