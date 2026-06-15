//! The `#[component]` attribute macro implementation.

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    AttrStyle, Attribute, Error, Expr, FnArg, Generics, Ident, Item, ItemFn, Meta, Pat, PatIdent,
    Result, ReturnType, Signature, Token, Type, TypeTuple, parse_quote,
};

pub struct ComponentFn {
    pub f: ItemFn,
}

impl Parse for ComponentFn {
    fn parse(input: ParseStream) -> Result<Self> {
        // Parse macro body.
        let parsed: Item = input.parse()?;

        match parsed {
            Item::Fn(mut f) => {
                let ItemFn { sig, .. } = &mut f;

                if sig.constness.is_some() {
                    return Err(syn::Error::new(
                        sig.constness.span(),
                        "const functions can't be components",
                    ));
                }

                if sig.abi.is_some() {
                    return Err(syn::Error::new(
                        sig.abi.span(),
                        "extern functions can't be components",
                    ));
                }

                if let ReturnType::Default = sig.output {
                    return Err(syn::Error::new(
                        sig.paren_token.span.close(),
                        "component must return `sycamore::view::View`",
                    ));
                };

                let inputs = sig.inputs.clone().into_iter().collect::<Vec<_>>();

                match &inputs[..] {
                    [] => {}
                    [input] => {
                        if let FnArg::Receiver(_) = input {
                            return Err(syn::Error::new(
                                input.span(),
                                "components can't accept a receiver",
                            ));
                        }

                        if let FnArg::Typed(pat) = input
                            && let Type::Tuple(TypeTuple { elems, .. }) = &*pat.ty
                            && elems.is_empty()
                        {
                            return Err(syn::Error::new(
                                pat.ty.span(),
                                "taking an unit tuple as props is useless",
                            ));
                        }
                    }
                    [..] => {
                        if inputs.len() > 1 {
                            return Err(syn::Error::new(
                                sig.inputs
                                    .clone()
                                    .into_iter()
                                    .skip(2)
                                    .collect::<Punctuated<_, Token![,]>>()
                                    .span(),
                                "component should not take more than 1 parameter",
                            ));
                        }
                    }
                };

                Ok(Self { f })
            }
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
        let ItemFn {
            attrs,
            vis,
            sig,
            block,
        } = &f;

        if sig.asyncness.is_some() {
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
                #[::sycamore::component]
                #vis #non_async_sig {
                    // Define the original function as a nested function so that it cannot be
                    // called from outside.
                    #[allow(non_snake_case)]
                    #inner_sig #block

                    ::sycamore::rt::WrapAsync(move || #inner_ident(#(#args),*))
                }
            });
        } else {
            tokens.extend(quote! {
                #[allow(non_snake_case)]
                #f
            })
        }
    }
}

/// Arguments to the `component` attribute proc-macro.
pub struct ComponentArgs {
    inline_props: Option<Ident>,
    _comma: Option<Token![,]>,
    attrs: Punctuated<Meta, Token![,]>,
}

impl Parse for ComponentArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let inline_props: Option<Ident> = input.parse()?;
        let (comma, attrs) = if let Some(inline_props) = &inline_props {
            // Check if the ident is correct.
            if *inline_props != "inline_props" {
                return Err(Error::new(inline_props.span(), "expected `inline_props`"));
            }

            let comma: Option<Token![,]> = input.parse()?;
            let attrs: Punctuated<Meta, Token![,]> = if comma.is_some() {
                input.parse_terminated(Meta::parse, Token![,])?
            } else {
                Punctuated::new()
            };
            (comma, attrs)
        } else {
            (None, Punctuated::new())
        };
        Ok(Self {
            inline_props,
            _comma: comma,
            attrs,
        })
    }
}

pub fn component_impl(args: ComponentArgs, item: TokenStream) -> Result<TokenStream> {
    if args.inline_props.is_some() {
        let mut item_fn = syn::parse::<ItemFn>(item.into())?;
        let inline_props = inline_props_impl(&mut item_fn, args.attrs)?;
        // TODO: don't parse the function twice.
        let comp = syn::parse::<ComponentFn>(item_fn.to_token_stream().into())?;
        Ok(quote! {
            #inline_props
            #comp
        })
    } else {
        let comp = syn::parse::<ComponentFn>(item.into())?;
        Ok(comp.to_token_stream())
    }
}

/// Codegens the new props struct and modifies the component body to accept this new struct as
/// props.
fn inline_props_impl(item: &mut ItemFn, attrs: Punctuated<Meta, Token![,]>) -> Result<TokenStream> {
    let props_vis = &item.vis;
    let props_struct_ident = format_ident!("{}_Props", item.sig.ident);

    let inputs = item.sig.inputs.clone();
    let props = inputs.clone().into_iter().collect::<Vec<_>>();
    let generics: &mut Generics = &mut item.sig.generics;
    let mut fields = Vec::new();
    for arg in inputs {
        match arg {
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new(
                    receiver.span(),
                    "`self` cannot be a property",
                ));
            }
            FnArg::Typed(pat_type) => match *pat_type.pat {
                Pat::Ident(ident_pat) => super::inline_props::push_field(
                    &mut fields,
                    generics,
                    pat_type.attrs,
                    ident_pat.clone().ident,
                    *pat_type.ty,
                ),
                _ => {
                    return Err(syn::Error::new(
                        pat_type.pat.span(),
                        "pattern must contain an identifier, properties cannot be unnamed",
                    ));
                }
            },
        }
    }

    let generics_phantoms = generics.params.iter().enumerate().filter_map(|(i, param)| {
        let phantom_ident = format_ident!("__phantom{i}");
        match param {
            syn::GenericParam::Type(ty) => {
                let ty = &ty.ident;
                Some(quote! {
                    #[prop(default, setter(skip))]
                    #phantom_ident: ::std::marker::PhantomData<#ty>
                })
            }
            syn::GenericParam::Lifetime(lt) => {
                let lt = &lt.lifetime;
                Some(quote! {
                    #[prop(default, setter(skip))]
                    #phantom_ident: ::std::marker::PhantomData<&#lt ()>
                })
            }
            syn::GenericParam::Const(_) => None,
        }
    });

    let doc_comment = format!("Props for [`{}`].", item.sig.ident);

    let attrs = attrs.into_iter().map(|a| Attribute {
        pound_token: Token![#](Span::call_site()),
        style: AttrStyle::Outer,
        bracket_token: Default::default(),
        meta: a,
    });
    let ret = Ok(quote! {
        #[allow(non_camel_case_types)]
        #[doc = #doc_comment]
        #[derive(::sycamore::rt::Props)]
        #(#attrs)*
        #props_vis struct #props_struct_ident #generics {
            #(#fields,)*
            #(#generics_phantoms,)*
        }
    });

    // Rewrite component body.

    // Get the ident (technically, patterns) of each prop.
    let props_pats = props.iter().map(|arg| match arg {
        FnArg::Receiver(_) => unreachable!(),
        FnArg::Typed(arg) => match &*arg.pat {
            Pat::Ident(pat) => {
                if pat.subpat.is_some() {
                    let ident = &pat.ident;
                    quote! { #ident: #pat }
                } else {
                    quote! { #pat }
                }
            }
            _ => unreachable!(),
        },
    });
    // Rewrite function signature.
    let props_struct_generics = generics.split_for_impl().1;
    item.sig.inputs = parse_quote! { __props: #props_struct_ident #props_struct_generics };
    // Rewrite function body.
    let block = item.block.clone();
    item.block = parse_quote! {{
        let #props_struct_ident {
            #(#props_pats,)*
            ..
        } = __props;
        #block
    }};

    ret
}
