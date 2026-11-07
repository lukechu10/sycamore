//! The `#[component]` attribute macro implementation.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{FnArg, ItemFn, Macro, Meta, Pat, Result, Signature, Token, parse_quote};

fn to_tokens(args: Punctuated<Macro, Comma>, mut f: ItemFn) -> TokenStream {
    f.attrs.push(parse_quote!(#[allow(non_snake_case)]));

    // Change all `#[prop]` attributes to `#[builder]` attributes so that the builder macro can
    // process them.
    for input in &mut f.sig.inputs {
        let attrs = match input {
            FnArg::Typed(pat_type) => &mut pat_type.attrs,
            FnArg::Receiver(receiver) => &mut receiver.attrs,
        };
        for attr in attrs {
            if attr.path().is_ident("prop") {
                match &mut attr.meta {
                    Meta::Path(path) => {
                        *path = parse_quote!(builder);
                    }
                    Meta::List(meta_list) => {
                        meta_list.path = parse_quote!(builder);
                    }
                    Meta::NameValue(meta_name_value) => {
                        meta_name_value.path = parse_quote!(builder);
                    }
                }
            }
        }
    }

    let ItemFn {
        attrs,
        vis,
        sig,
        block,
        ..
    } = &f;

    if f.sig.asyncness.is_some() {
        let inputs = &sig.inputs;
        let args = inputs
            .iter()
            .map(|fn_arg| match fn_arg {
                FnArg::Typed(t) => match &*t.pat {
                    Pat::Ident(id) => &id.ident,
                    _ => panic!(
                        "async component props cannot be destructured in the function signature"
                    ),
                },
                FnArg::Receiver(_) => unreachable!(),
            })
            .collect::<Vec<_>>();

        let non_async_sig = Signature {
            asyncness: None,
            ..sig.clone()
        };
        let inner_ident = format_ident!("{}_async", sig.ident);
        let inner_sig = Signature {
            ident: inner_ident.clone(),
            ..sig.clone()
        };
        quote! {
            // Create a new function that is not async so that it is just a standard component.
            #(#attrs)*
            // TODO: this is a hack to get the builder macro to work with async components.
            // We plan on removing async components in the future so this is good enough for now.
            #[builder]
            #vis #non_async_sig {
                // Define the original function as a nested function so that it cannot be
                // called from outside.
                #[allow(non_snake_case)]
                #inner_sig #block

                ::sycamore::rt::wrap_async_component(move || #inner_ident(#(#args),*))
            }
        }
    } else {
        // If the builder macro failed to expand, change the `#[builder]` attribute in the error
        // message back to `#[prop]` so that the user can see the original attribute that caused
        // the error.
        let builder_expansion = crate::builder::expand(f);
        let builder_expansion = match builder_expansion {
            Ok(expansion) => expansion,
            Err(err) => {
                let original_err = err.to_compile_error();
                let message = original_err
                    .to_string()
                    .split('\"')
                    .nth(1)
                    .unwrap_or("")
                    .replace("#[builder", "#[prop");
                return syn::Error::new_spanned(original_err, message).to_compile_error();
            }
        };
        let builder = builder_expansion.tokens;

        let macros = args.into_iter().map(|mut _macro| {
            // Prepend each macro with the builder information so that the macro can access the
            // builder type and generics.
            let macro_input = _macro.tokens.clone();
            let (impl_generics, ty_generics, where_clause) =
                &builder_expansion.builder.generics.split_for_impl();
            let ty_ident = &builder_expansion.builder.ident;
            _macro.tokens = quote! {
                builder: {
                    ty: #ty_ident #ty_generics,
                    impl_generics: [#impl_generics],
                    where_clause: [#where_clause],
                },
                #macro_input
            };
            // Change the macro delimiter to braces so that we don't run into missing semicolon
            // errors.
            _macro.delimiter = syn::MacroDelimiter::Brace(syn::token::Brace {
                span: *_macro.delimiter.span(),
            });
            _macro
        });

        quote! {
            #builder

            #(#macros)*
        }
    }
}

/// Arguments to the `component` attribute proc-macro.
pub struct ComponentArgs {
    _comma: Option<Token![,]>,
    attrs: Punctuated<Macro, Token![,]>,
}

impl Parse for ComponentArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let comma: Option<Token![,]> = input.parse()?;
        let attrs: Punctuated<Macro, Token![,]> = if !input.is_empty() {
            input.parse_terminated(Macro::parse, Token![,])?
        } else {
            Punctuated::new()
        };
        Ok(Self {
            _comma: comma,
            attrs,
        })
    }
}

pub fn expand(args: ComponentArgs, f: ItemFn) -> Result<TokenStream> {
    Ok(to_tokens(args.attrs, f))
}
