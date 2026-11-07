//! The `#[component]` attribute macro implementation.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Expr, FnArg, Ident, Item, ItemFn, Meta, Pat, PatIdent, Result, Signature, Token, parse_quote,
};

pub struct ComponentFn {
    pub f: ItemFn,
}

impl Parse for ComponentFn {
    fn parse(input: ParseStream) -> Result<Self> {
        // Parse macro body.
        let parsed: Item = input.parse()?;

        match parsed {
            Item::Fn(f) => Ok(Self { f }),
            item => Err(syn::Error::new_spanned(
                item,
                "the `component` attribute can only be applied to functions",
            )),
        }
    }
}

struct AsyncCompInputs {
    sync_input: Punctuated<FnArg, Token![,]>,
    async_args: Vec<Expr>,
}

fn async_comp_inputs_from_sig_inputs(inputs: &Punctuated<FnArg, Token![,]>) -> AsyncCompInputs {
    let mut sync_input = Punctuated::new();
    let mut async_args = Vec::new();

    fn pat_ident_arm(
        sync_input: &mut Punctuated<FnArg, Token![,]>,
        fn_arg: &FnArg,
        id: &PatIdent,
    ) -> Expr {
        sync_input.push(fn_arg.clone());
        let ident = &id.ident;
        parse_quote! { #ident }
    }

    let mut inputs = inputs.iter();

    let prop_arg = inputs.next();
    let prop_arg = prop_arg.map(|prop_fn_arg| match prop_fn_arg {
        FnArg::Typed(t) => match &*t.pat {
            Pat::Ident(id) => pat_ident_arm(&mut sync_input, prop_fn_arg, id),
            Pat::Struct(pat_struct) => {
                // For the sync input we don't want a destructured pattern but just to take a
                // `syn::PatType` (i.e. `props: MyPropsStruct`) then the inner async function
                // signature can have the destructured pattern and it will work correctly
                // as long as we provide our brand new ident that we used in the
                // `syn::PatIdent`.
                let ident = Ident::new("props", pat_struct.span());
                // Props are taken by value so no refs or mutability required here
                // The destructured pattern can add mutability (if required) even without this
                // set.
                let pat_ident = PatIdent {
                    attrs: vec![],
                    by_ref: None,
                    mutability: None,
                    ident,
                    subpat: None,
                };
                let pat_type = syn::PatType {
                    attrs: vec![],
                    pat: Box::new(Pat::Ident(pat_ident)),
                    colon_token: Default::default(),
                    ty: t.ty.clone(),
                };

                let fn_arg = FnArg::Typed(pat_type);
                sync_input.push(fn_arg);
                parse_quote! { props }
            }
            _ => panic!("unexpected pattern!"),
        },
        FnArg::Receiver(_) => unreachable!(),
    });

    if let Some(arg) = prop_arg {
        async_args.push(arg);
    }

    AsyncCompInputs {
        async_args,
        sync_input,
    }
}

impl ToTokens for ComponentFn {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ComponentFn { f } = self;
        let mut f = f.clone();

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
            // When the component function is async then we need to extract out some of the
            // function signature (Syn::Signature) so that we can wrap the async function with
            // a non-async component.
            //
            // In order to support the struct destructured pattern for props we alter the existing
            // signature for the non-async component so that it is defined as a `Syn::PatType`
            // (i.e. props: MyPropsStruct) with a new `Syn::Ident` "props". We then use this ident
            // again as an argument to the inner async function which has the user defined
            // destructured pattern which will work as expected.
            //
            // Note: this does not affect the signature of the function.
            let inputs = &sig.inputs;
            let AsyncCompInputs {
                sync_input,
                async_args: args,
            } = async_comp_inputs_from_sig_inputs(inputs);

            let non_async_sig = Signature {
                asyncness: None,
                inputs: sync_input,
                ..sig.clone()
            };
            let inner_ident = format_ident!("{}_inner", sig.ident);
            let inner_sig = Signature {
                ident: inner_ident.clone(),
                ..sig.clone()
            };
            tokens.extend(quote! {
                // Create a new function that is not async so that it is just a standard component.
                #(#attrs)*
                // TODO: this is a hack to get the builder macro to work with async components.
                // We plan on removing async components in the future so this is good enough for now.
                #[builder]
                #[allow(non_snake_case)]
                #vis #non_async_sig {
                    // Define the original function as a nested function so that it cannot be
                    // called from outside.
                    #[allow(non_snake_case)]
                    #inner_sig #block

                    ::sycamore::rt::wrap_async_component(move || #inner_ident(#(#args),*))
                }
            });
        } else {
            // If the builder macro failed to expand, change the `#[builder]` attribute in the error
            // message back to `#[prop]` so that the user can see the original attribute that caused
            // the error.
            let builder_impl = crate::builder::expand(f);
            let builder_impl = match builder_impl {
                Ok(tokens) => tokens,
                Err(err) => {
                    let original_err = err.to_compile_error();
                    let message = original_err
                        .to_string()
                        .split('\"')
                        .nth(1)
                        .unwrap_or("")
                        .replace("#[builder", "#[prop");
                    tokens
                        .extend(syn::Error::new_spanned(original_err, message).to_compile_error());
                    return;
                }
            };
            tokens.extend(builder_impl)
        }
    }
}

/// Arguments to the `component` attribute proc-macro.
pub struct ComponentArgs {
    _comma: Option<Token![,]>,
    attrs: Punctuated<Meta, Token![,]>,
}

impl Parse for ComponentArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let comma: Option<Token![,]> = input.parse()?;
        let attrs: Punctuated<Meta, Token![,]> = if !input.is_empty() {
            input.parse_terminated(Meta::parse, Token![,])?
        } else {
            Punctuated::new()
        };
        Ok(Self {
            _comma: comma,
            attrs,
        })
    }
}

pub fn component_impl(args: ComponentArgs, item: TokenStream) -> Result<TokenStream> {
    if !args.attrs.is_empty() {
        return Err(syn::Error::new_spanned(
            args.attrs,
            "the `#[component]` attribute does not accept any arguments",
        ));
    }
    let comp = syn::parse::<ComponentFn>(item.into())?;
    Ok(comp.into_token_stream())
}
