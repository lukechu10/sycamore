use proc_macro2::TokenStream;
use quote::{ToTokens, quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{DeriveInput, Fields, Ident, LitStr, Token, Variant};

use crate::parser::{RoutePathAst, SegmentAst, parse_route};

pub fn route_impl(input: DeriveInput) -> syn::Result<TokenStream> {
    let mut quoted = TokenStream::new();
    let mut err_quoted = TokenStream::new();
    // When the `#[not_found]` handler is found, this will store its name so we can use that as the
    // `Default` implementation
    let mut error_handler_name = None;

    match &input.data {
        syn::Data::Enum(de) => {
            let ty_name = &input.ident;

            for variant in &de.variants {
                let variant_id = &variant.ident;

                let mut quote_capture_vars = TokenStream::new();
                let mut route_path_ast = None;

                let mut is_to_route = false;

                for attr in &variant.attrs {
                    let attr_name = match attr.path().get_ident() {
                        Some(ident) => ident.to_string(),
                        None => continue,
                    };

                    match attr_name.as_str() {
                        "to" => {
                            // region: parse route
                            let route_litstr: LitStr = attr.parse_args()?;
                            let route_str = route_litstr.value();
                            let route = match parse_route(&route_str) {
                                Ok(route_ast) => route_ast,
                                Err(err) => {
                                    return Err(syn::Error::new(route_litstr.span(), err.message));
                                }
                            };
                            // endregion
                            quote_capture_vars.extend(impl_to(variant, variant_id, &route)?);
                            route_path_ast = Some(route);
                            is_to_route = true;
                        }
                        "not_found" => {
                            if error_handler_name.is_some() {
                                return Err(syn::Error::new(
                                    attr.span(),
                                    "cannot have more than one error handler",
                                ));
                            }
                            if !variant.fields.is_empty() {
                                return Err(syn::Error::new(
                                    variant.fields.span(),
                                    "not found route cannot have any fields",
                                ));
                            }
                            err_quoted = quote! {
                                return Self::#variant_id;
                            };
                            error_handler_name = Some(quote!(Self::#variant_id));
                        }
                        _ => {}
                    }
                }
                if is_to_route {
                    let route_path_ast = route_path_ast.unwrap();
                    quoted.extend(quote! {
                        let __route = #route_path_ast;
                        if let Some(__captures) = __route.match_path(__segments) {
                            // Try to capture variables.
                            #quote_capture_vars
                        }
                    });
                }
            }

            if error_handler_name.is_none() {
                return Err(syn::Error::new(
                    input.span(),
                    "not found route not specified",
                ));
            }

            Ok(quote! {
                impl ::sycamore_router::Route for #ty_name {
                    fn match_route(&self, __segments: &[&str]) -> Self {
                        #quoted
                        #err_quoted
                    }
                }
                // We implement `Default` as well here for the `Router`/`RouterBase` distinction (`Router` needs to pass a default `impl Route` to `RouterBase`)
                impl ::std::default::Default for #ty_name {
                    fn default() -> Self {
                        #error_handler_name
                    }
                }
            })
        }
        _ => Err(syn::Error::new(
            input.span(),
            "Route can only be derived on enums",
        )),
    }
}

/// Implementation for `#[to(_)]` attribute.
fn impl_to(
    variant: &Variant,
    variant_id: &Ident,
    route: &RoutePathAst,
) -> Result<TokenStream, syn::Error> {
    let dyn_segments = route.dyn_segments();
    let expected_fields_len = dyn_segments.len();
    if expected_fields_len != variant.fields.len() {
        return Err(syn::Error::new(
            variant.fields.span(),
            format!(
                "mismatch between number of capture fields and variant fields (found {} capture field(s) and {} variant field(s))",
                expected_fields_len,
                variant.fields.len()
            ),
        ));
    }

    Ok(match &variant.fields {
        // For named fields, captures must match the field name.
        Fields::Named(f) => {
            let mut captures = Vec::new();

            for (i, (field, segment)) in f.named.iter().zip(dyn_segments.iter()).enumerate() {
                match segment {
                    SegmentAst::Param(_) => unreachable!("not a dynamic segment"),
                    SegmentAst::DynParam(param) => {
                        if param != &field.ident.as_ref().unwrap().to_string() {
                            return Err(syn::Error::new(
                                field.ident.span(),
                                format!(
                                    "capture field name mismatch (expected `{}`, found `{}`)",
                                    param,
                                    field.ident.as_ref().unwrap()
                                ),
                            ));
                        }
                        let param_id: Ident = syn::parse_str(param)?;
                        captures.push(quote! {
                            let #param_id = match ::sycamore_router::TryFromParam::try_from_param(
                                __captures[#i].as_dyn_param().unwrap()
                            ) {
                                ::std::option::Option::Some(__value) => __value,
                                ::std::option::Option::None => break,
                            };
                        })
                    }
                    SegmentAst::DynSegments(param) => {
                        if param != &field.ident.as_ref().unwrap().to_string() {
                            return Err(syn::Error::new(
                                field.ident.span(),
                                format!(
                                    "capture field name mismatch (expected `{}`, found `{}`)",
                                    param,
                                    field.ident.as_ref().unwrap()
                                ),
                            ));
                        }
                        let param_id: Ident = syn::parse_str(param)?;
                        captures.push(quote! {
                            let #param_id = match ::sycamore_router::TryFromSegments::try_from_segments(
                                __captures[#i].as_dyn_segments().unwrap()
                            ) {
                                ::std::option::Option::Some(__value) => __value,
                                ::std::option::Option::None => break,
                            };
                        })
                    }
                }
            }
            let named: Punctuated<&Option<Ident>, Token![,]> =
                f.named.iter().map(|x| &x.ident).collect();
            quote_spanned! {variant.span()=>
                #[allow(clippy::never_loop)]
                #[allow(clippy::while_let_loop)]
                loop {
                    #(#captures)*
                    return Self::#variant_id {
                        #named
                    };
                }
            }
        }
        // For unnamed fields, captures must be in right order.
        Fields::Unnamed(_) => {
            let mut captures = Vec::new();

            for (i, segment) in dyn_segments.iter().enumerate() {
                match segment {
                    SegmentAst::Param(_) => unreachable!("not a dynamic segment"),
                    SegmentAst::DynParam(_) => captures.push(quote! {{
                        match ::sycamore_router::TryFromParam::try_from_param(
                            __captures[#i].as_dyn_param().unwrap()
                        ) {
                            ::std::option::Option::Some(__value) => __value,
                            ::std::option::Option::None => break,
                        }
                    }}),
                    SegmentAst::DynSegments(_) => captures.push(quote! {{
                        match ::sycamore_router::TryFromSegments::try_from_segments(
                            __captures[#i].as_dyn_segments().unwrap()
                        ) {
                            ::std::option::Option::Some(__value) => __value,
                            ::std::option::Option::None => break,
                        }
                    }}),
                }
            }
            quote! {
                // Run captures inside a loop in order to allow early break inside the expression.
                loop {
                    return Self::#variant_id(#(#captures),*);
                }
            }
        }
        Fields::Unit => quote! {
            return Self::#variant_id;
        },
    })
}

impl ToTokens for SegmentAst {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            SegmentAst::Param(param) => tokens.extend(quote! {
                ::sycamore_router::Segment::Param(::std::string::ToString::to_string(#param))
            }),
            SegmentAst::DynParam(_) => tokens.extend(quote! {
                ::sycamore_router::Segment::DynParam
            }),
            SegmentAst::DynSegments(_) => tokens.extend(quote! {
                ::sycamore_router::Segment::DynSegments
            }),
        }
    }
}

impl ToTokens for RoutePathAst {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let segments = self
            .segments
            .iter()
            .map(|s| s.to_token_stream())
            .collect::<Vec<_>>();

        tokens.extend(quote! {
            ::sycamore_router::RoutePath::new(::std::vec![#(#segments),*])
        });
    }
}
