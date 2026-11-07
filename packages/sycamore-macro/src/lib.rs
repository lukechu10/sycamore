//! Proc-macros used in [Sycamore](https://docs.rs/sycamore).
//!
//! This crate is re-exported in the [`sycamore`](https://docs.rs/sycamore) crate, so you can use the proc-macros by importing the `sycamore` crate instead of this crate directly.

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
/// To learn more about components, see the chapter on
/// [components](https://sycamore.dev/book/introduction/your-first-app#using-components) in the Sycamore Book.
///
/// Behind the scenes, this macro generates a builder struct for the component function, which
/// allows for setting the component's props using a fluent builder pattern. This also allows for
/// things like optional or default props.
///
/// # Optional props
///
/// By default, any prop with a type `Option<T>` is automatically optional and does not need to be
/// set when creating the component. If the prop is not set, it will be `None` in the component
/// function.
///
/// To prevent a prop from being optional, you can use the `#[prop(required)]` attribute on the
/// prop.
///
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn MyComponent(optional: Option<String>, #[prop(required)] required: Option<String>) -> View {
///     // ...
/// #   todo!()
/// }
/// ```
///
/// Note that optional props also strip the `Option` type from the prop in the builder. For
/// example, in the above example:
/// ```
/// # use sycamore::prelude::*;
/// # #[component]
/// # fn MyComponent(optional: Option<String>, #[prop(required)] required: Option<String>) -> View { todo!() }
/// # fn __() -> View {
/// view! {
///     // optional is set to Some("Hello")
///     MyComponent(optional="Hello".to_string(), required=Some("World".to_string()))
///
///     // optional is set to None
///     MyComponent(required=Some("World".to_string()))
/// }
/// # }
/// ```
///
/// # Default props
///
/// You can also specify a default value for a prop using the `#[prop(default = <value>)]`
/// attribute. If the prop is not explicitly set, the default value will be used instead.
/// If no value is specified, the default value will be `Default::default()`.
///
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn CustomButton(
///     /// The disabled state of the button. Defaults to `false`.
///     #[prop(default = false)] disabled: bool,
///     /// The class of the button. Defaults to an empty string.
///     #[prop(default)] class: String,
/// ) -> View {
///     // ...
/// #   todo!()
/// }
/// ```
///
/// # Automatic prop conversion
///
/// Adding `#[prop(into)]` to a prop will automatically convert the value passed to the prop into
/// the builder using the [`Into`] trait.
///
/// # Inferred default and into attributes
///
/// On certain types, the `#[prop(default)]` and `#[prop(into)]` attributes are automatically
/// inferred. These types are:
/// - `Attributes`: represents a set of attributes that can be passed to an HTML element.
/// - `Children`: represents the children of a component.
/// - `Callback`: represents a callback function that can be passed to a component.
///
/// In such cases, it is almost always desired to have the `#[prop(default)]` and `#[prop(into)]`
/// attributes automatically applied, so they are inferred by default. However, if you want to
/// disable this behavior, you can use `#[prop(required)]` to explicitly disable the `default` and
/// `into` attributes for a prop. If you want to disable only the `default` attribute but not the
/// `into` attribute, you can use
/// `#[prop(required, into)]`.
#[proc_macro_attribute]
pub fn component(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as component::ComponentArgs);
    let f = {
        let item = item.clone();
        parse_macro_input!(item as syn::ItemFn)
    };

    component::expand(args, f)
        .unwrap_or_else(|err| {
            // If proc-macro errors, emit the original function for better IDE support.
            let error_tokens = err.into_compile_error();
            let item: proc_macro2::TokenStream = item.into();
            quote! {
                #item
                #error_tokens
            }
        })
        .into()
}

/// Replaces a function with a zero-argument fluent builder constructor.
///
/// This is primarily used by the [`#[component]`](macro@component) macro to generate a builder for
/// the component function. However, it can also be used directly on a function to generate a
/// builder for it.
///
/// There are a few differences between this macro and the `#[component]` macro.
/// In particular:
/// - This macro uses `#[builder]` attributes on the function parameters instead of `#[prop]`
///   attributes.
/// - This macro does not automatically infer `#[builder(default)]` or `#[builder(into)]` attributes
///   on certain types unlike `#[component]`. You must explicitly specify these attributes if you
///   want them.
///
/// Besides these differences, the `#[builder]` macro behaves identically to the `#[component]`
/// macro in terms of generating a builder for the function, since they both use the same underlying
/// builder generation code.
///
/// # Example
/// ```
/// # use sycamore_macro::*;
/// #[builder]
/// fn my_function(
///     #[builder(default = 42)] x: i32,
///     #[builder(required)] y: Option<i32>,
///     #[builder(into)] z: String,
/// ) -> i32 {
///     x + y.unwrap_or(0) + z.len() as i32
/// }
/// let result = my_function().x(10).y(Some(5)).z("Hello").build();
/// assert!(result == 10 + 5 + 5);
/// ```
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
