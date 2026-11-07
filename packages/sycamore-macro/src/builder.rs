//! Automatically convert function arguments into a fluent API for Sycamore components.
//!
//! It supports synchronous and async crate-local free functions with named arguments, generic
//! parameters, and argument- and return-position `impl Trait`. `Option<T>`,
//! `std::option::Option<T>`, and `core::option::Option<T>` arguments are optional; aliases are not
//! recognized and an unqualified `Option` must refer to the standard type.
//!
//! `#[builder(required)]` on an automatically optional parameter requires its setter to receive the
//! full `Option<T>` (including `None`).
//!
//! `#[builder(default)]` uses `Default::default`, while `#[builder(default = callable)]` invokes a
//! zero-argument callable. Defaults are evaluated at build time only if unset, and a defaulted
//! `Option<T>` setter takes the full `Option<T>`. `default` and `required` cannot be combined.
//! Required setters may only be called once; optional and defaulted setters replace earlier values.
//! `#[builder(into)]` makes a setter accept `impl Into<SetterType>`, where `SetterType` is the type
//! the setter would otherwise accept.
//! Other argument attributes, including doc comments, are forwarded unchanged to the corresponding
//! builder setter method, not retained on the private helper's parameters.
//! Argument-position `impl Trait` introduces an inferred constructor type parameter; callers
//! specifying explicit type arguments may need an additional `_`. An unset `Option<impl Trait>` may
//! require an explicit type argument. Opaque return types use a function pointer in the builder and
//! still implement `Build<T>`.
//! Async functions have synchronous constructors and setters; `build()` returns an unboxed future
//! and implements `Build<FutureType>`. Defaults are evaluated when `build()` is called, before the
//! future is polled. No `Send` or `'static` bounds are added.
//!
//! The builder, helpers, and implementations stay in the function's original scope,
//! including for nested functions. Expansion creates a builder named `__builder_<function>`,
//! a private marker module named `__builder_<function>_fields`, and a private
//! `__builder_<function>_inner` helper. A function with defaults also gets a
//! `__builder_<function>_defaults` helper. These names must not collide with existing items.
//! Only field markers live in the module, using argument names for readable diagnostics.
//! No path rebasing is needed.
//! The original function name becomes the builder constructor, so recursive calls to that name in
//! the original body are not supported.
//!
//! `cfg`, `allow`, `warn`, `deny`, and `forbid` attributes are forwarded to all generated
//! top-level items. `cfg_attr` is filtered recursively to forward only these attributes.
//! All function attributes, including `expect`, remain on the builder constructor.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::{
    FnArg, GenericArgument, Ident, ItemFn, Pat, PathArguments, ReturnType, Type, Visibility,
    parse_quote,
};

struct Field {
    attrs: Vec<syn::Attribute>,
    ident: Ident,
    ty: Type,
    optional_inner: Option<Type>,
    default: Option<DefaultValue>,
    into: bool,
    state: Option<Ident>,
}

enum DefaultValue {
    Trait,
    Function(syn::Expr),
}

#[derive(Default)]
struct ParameterOptions {
    required: bool,
    default: Option<DefaultValue>,
    into: bool,
}

/// Generated code and the metadata needed to extend its builder.
pub struct BuilderExpansion {
    pub tokens: TokenStream2,
    // The standalone attribute only needs `tokens`; component macros use this metadata.
    pub builder: BuilderInfo,
}

/// Describes the builder in every required-field state, not just its initial state.
pub struct BuilderInfo {
    pub ident: Ident,
    /// Complete builder parameters and original constraints, including inferred input types,
    /// lifetimes, opaque output types, and required-field states. Use `split_for_impl()`
    /// to generate an extension impl without adding build-only bounds such as `Default`.
    pub generics: syn::Generics,
}

pub fn expand(mut function: ItemFn) -> syn::Result<BuilderExpansion> {
    let signature = &function.sig;
    if signature.constness.is_some()
        || !matches!(signature.safety, syn::Safety::Default)
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(syn::Error::new_spanned(
            signature,
            "`#[builder]` supports only safe, non-const Rust functions",
        ));
    }

    let helper_generics = signature.generics.clone();
    let mut root_generics = helper_generics.clone();
    let mut lowered_inputs = signature.inputs.clone();
    let mut impl_lowerer = ImplArgumentLowerer::new(&root_generics);
    for arg in &mut lowered_inputs {
        if let FnArg::Typed(arg) = arg {
            impl_lowerer.visit_type_mut(&mut arg.ty);
        }
    }
    root_generics.params.extend(impl_lowerer.params);
    let mut lifetimes = InputLifetimes::new(&root_generics);
    for arg in &lowered_inputs {
        if let FnArg::Typed(arg) = arg {
            lifetimes.reserve_names(&arg.ty);
        }
    }
    for arg in &mut lowered_inputs {
        if let FnArg::Typed(arg) = arg {
            lifetimes.visit_type_mut(&mut arg.ty);
        }
    }
    for lifetime in lifetimes.generated.iter().rev() {
        root_generics.params.insert(0, parse_quote!(#lifetime));
    }
    let mut lowered_output = signature.output.clone();
    if let ReturnType::Type(_, ty) = &mut lowered_output {
        let inferred = if lifetimes.seen.len() == 1 {
            Some(lifetimes.seen[0].clone())
        } else {
            None
        };
        let mut output_lifetimes = OutputLifetimes {
            inferred,
            error: None,
        };
        output_lifetimes.visit_type_mut(ty);
        if let Some(error) = output_lifetimes.error {
            return Err(error);
        }
    }
    for param in &root_generics.params {
        let attrs = match param {
            syn::GenericParam::Lifetime(param) => &param.attrs,
            syn::GenericParam::Type(param) => &param.attrs,
            syn::GenericParam::Const(param) => &param.attrs,
        };
        if !attrs.is_empty() {
            return Err(syn::Error::new_spanned(
                param,
                "`#[builder]` does not support attributes on generic parameters",
            ));
        }
    }
    let generic_names: Vec<_> = root_generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(param) => Some(param.ident.clone()),
            syn::GenericParam::Const(param) => Some(param.ident.clone()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect();
    let name = signature.ident.clone();
    let name_suffix = name.to_string();
    let name_suffix = name_suffix.trim_start_matches("r#");
    let builder = format_ident!("__builder_{}", name_suffix);
    let fields_module = format_ident!("__builder_{}_fields", name_suffix);
    let mut fields = Vec::new();
    let mut required_count = 0usize;
    for arg in &lowered_inputs {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "`#[builder]` requires a free function",
            ));
        };
        let options = parameter_options(&arg.attrs)?;
        let Pat::Ident(pat) = arg.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "`#[builder]` requires named parameters",
            ));
        };
        if pat.by_ref.is_some() || pat.subpat.is_some() {
            return Err(syn::Error::new_spanned(
                pat,
                "`#[builder]` requires plain named parameters",
            ));
        }
        if pat.ident == "build" {
            return Err(syn::Error::new_spanned(
                &pat.ident,
                "a builder parameter cannot be named `build`",
            ));
        }
        validate_type(&arg.ty, false)?;
        let ty = (*arg.ty).clone();
        let automatic_optional = option_inner(&ty);
        if options.required && options.default.is_some() {
            return Err(syn::Error::new_spanned(
                arg,
                "`#[builder(required)]` and `#[builder(default)]` cannot be combined",
            ));
        }
        if options.required && automatic_optional.is_none() {
            return Err(syn::Error::new_spanned(
                &arg.ty,
                "`#[builder(required)]` can only be used on an automatically optional `Option<T>` parameter",
            ));
        }
        let is_required =
            options.required || (automatic_optional.is_none() && options.default.is_none());
        let optional_inner = if is_required || options.default.is_some() {
            None
        } else {
            automatic_optional
        };
        let state = if is_required {
            let state = loop {
                let state = format_ident!("__BuilderField{}", required_count);
                required_count += 1;
                if !generic_names.contains(&state) {
                    break state;
                }
            };
            Some(state)
        } else {
            None
        };
        fields.push(Field {
            attrs: arg
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("builder"))
                .cloned()
                .collect(),
            ident: pat.ident.clone(),
            ty,
            optional_inner,
            default: options.default,
            into: options.into,
            state,
        });
    }
    let root_output: Type = match &lowered_output {
        ReturnType::Default => parse_quote!(()),
        ReturnType::Type(_, ty) => {
            validate_type(ty, true)?;
            (**ty).clone()
        }
    };
    let root_output: Type = if signature.asyncness.is_some() {
        parse_quote!(impl ::core::future::Future<Output = #root_output>)
    } else {
        root_output
    };
    let opaque_output = contains_impl_trait(&root_output);
    let mut output_ident = format_ident!("__BuilderOutput");
    while generic_names.contains(&output_ident) {
        output_ident = format_ident!("_{}", output_ident);
    }

    let helper = format_ident!(
        "__builder_{}_inner",
        name.to_string().trim_start_matches("r#")
    );
    let defaults_helper = format_ident!(
        "__builder_{}_defaults",
        name.to_string().trim_start_matches("r#")
    );
    let visibility = function.vis.clone();
    let attrs = function.attrs.clone();
    let shared_attrs: Vec<_> = attrs
        .iter()
        .filter_map(|attr| match shared_attribute_meta(&attr.meta) {
            Ok(Some(meta)) => Some(Ok(quote!(#[#meta]))),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<syn::Result<_>>()?;
    function.attrs.clear();
    function.vis = Visibility::Inherited;
    function.sig.ident = helper.clone();
    for arg in &mut function.sig.inputs {
        if let FnArg::Typed(arg) = arg {
            arg.attrs.clear();
        }
    }

    let required: Vec<_> = fields
        .iter()
        .filter(|field| field.state.is_some())
        .collect();
    let default_fields: Vec<_> = fields
        .iter()
        .filter(|field| field.default.is_some())
        .collect();
    let default_argument_names: Vec<_> = (0..default_fields.len())
        .map(|index| format_ident!("__builder_default_arg_{}", index, span = Span::mixed_site()))
        .collect();
    let default_value_names: Vec<_> = (0..default_fields.len())
        .map(|index| {
            format_ident!(
                "__builder_default_value_{}",
                index,
                span = Span::mixed_site()
            )
        })
        .collect();
    let states: Vec<_> = required
        .iter()
        .map(|field| field.state.as_ref().unwrap())
        .collect();
    let mut base_declarations: Vec<_> = root_generics
        .params
        .iter()
        .map(|param| quote!(#param))
        .collect();
    let mut base_arguments: Vec<_> = root_generics
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                quote!(#lifetime)
            }
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                quote!(#ident)
            }
            syn::GenericParam::Const(param) => {
                let ident = &param.ident;
                quote!({#ident})
            }
        })
        .collect();
    let root_base_arguments = base_arguments.clone();
    if opaque_output {
        base_declarations.push(quote!(#output_ident));
        base_arguments.push(quote!(#output_ident));
    }
    let helper_arguments: Vec<_> = helper_generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Lifetime(_) => None,
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                Some(quote!(#ident))
            }
            syn::GenericParam::Const(param) => {
                let ident = &param.ident;
                Some(quote!({#ident}))
            }
        })
        .collect();
    let helper_turbofish = if helper_arguments.is_empty() {
        quote!()
    } else {
        quote!(::<#(#helper_arguments),*>)
    };
    let constructor_arguments: Vec<_> = root_generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Lifetime(_) => None,
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                Some(quote!(#ident))
            }
            syn::GenericParam::Const(param) => {
                let ident = &param.ident;
                Some(quote!({#ident}))
            }
        })
        .collect();
    let default_turbofish = if constructor_arguments.is_empty() {
        quote!()
    } else {
        quote!(::<#(#constructor_arguments),*>)
    };
    let mut builder_generics = root_generics.clone();
    if opaque_output {
        builder_generics.params.push(parse_quote!(#output_ident));
    }
    for state in &states {
        builder_generics.params.push(parse_quote!(#state));
    }
    let builder_info = BuilderInfo {
        ident: builder.clone(),
        generics: builder_generics,
    };
    let (parameters, _, _) = builder_info.generics.split_for_impl();
    let type_parameters = angle(
        &base_arguments
            .iter()
            .cloned()
            .chain(states.iter().map(|state| quote!(#state)))
            .collect::<Vec<_>>(),
    );
    let missing: Vec<_> = required
        .iter()
        .map(|_| quote!(::sycamore::rt::Missing))
        .collect();
    let root_result_type = opaque_output.then(|| quote!(#root_output));
    let root_missing_parameters = angle(
        &root_base_arguments
            .iter()
            .cloned()
            .chain(root_result_type)
            .chain(missing.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let result_type = if opaque_output {
        quote!(#output_ident)
    } else {
        quote!(#root_output)
    };
    let root_where = &root_generics.where_clause;
    let original_predicates: Vec<_> = root_where
        .iter()
        .flat_map(|clause| clause.predicates.iter())
        .collect();
    let marker_elements: Vec<_> = root_generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Lifetime(param) => {
                let lifetime = &param.lifetime;
                Some(quote!(fn() -> &#lifetime ()))
            }
            syn::GenericParam::Type(param) => {
                let ident = &param.ident;
                Some(quote!(fn() -> *const #ident))
            }
            syn::GenericParam::Const(_) => None,
        })
        .collect();
    let field_names: Vec<_> = fields.iter().map(|field| &field.ident).collect();
    let mut marker_name = format_ident!("__builder_phantom");
    while field_names.contains(&&marker_name) {
        marker_name = format_ident!("_{}", marker_name);
    }
    let mut callback_name = format_ident!("__builder_callback");
    while field_names.contains(&&callback_name) || callback_name == marker_name {
        callback_name = format_ident!("_{}", callback_name);
    }
    let callback_types: Vec<_> = fields.iter().map(|field| &field.ty).collect();
    let callback_field = if opaque_output {
        quote!(#callback_name: fn(#(#callback_types),*) -> #output_ident,)
    } else {
        quote!()
    };
    let callback_transfer = if opaque_output {
        quote!(#callback_name: self.#callback_name,)
    } else {
        quote!()
    };
    let callback_initial = if opaque_output {
        quote!(#callback_name: #helper #helper_turbofish as fn(#(#callback_types),*) -> _,)
    } else {
        quote!()
    };
    let field_types: Vec<_> = fields
        .iter()
        .map(|field| match &field.state {
            Some(state) => quote!(#state),
            None => {
                let ty = &field.ty;
                if field.default.is_some() {
                    quote!(::core::option::Option<#ty>)
                } else {
                    quote!(#ty)
                }
            }
        })
        .collect();
    let initial_values: Vec<_> = fields
        .iter()
        .map(|field| {
            if field.state.is_some() {
                quote!(::sycamore::rt::Missing)
            } else {
                quote!(::core::option::Option::None)
            }
        })
        .collect();
    let markers = required.iter().map(|field| {
        let ident = &field.ident;
        quote! {
            #[allow(non_camel_case_types)]
            pub struct #ident;
        }
    });
    let mut bounds: Vec<_> = required
        .iter()
        .map(|field| {
            let state = field.state.as_ref().unwrap();
            let ident = &field.ident;
            let ty = &field.ty;
            quote!(#state: ::sycamore::rt::FieldValue<#fields_module::#ident, #ty>)
        })
        .collect();
    bounds.extend(default_fields.iter().filter_map(|field| {
        if matches!(field.default, Some(DefaultValue::Trait)) {
            let ty = &field.ty;
            Some(quote!(#ty: ::core::default::Default))
        } else {
            None
        }
    }));
    let where_clause = if bounds.is_empty() {
        quote!()
    } else {
        quote!(where #(#bounds),*)
    };
    let combined_where = if original_predicates.is_empty() && bounds.is_empty() {
        quote!()
    } else {
        quote!(where #(#original_predicates,)* #(#bounds),*)
    };
    let mut default_index = 0;
    let arguments = fields.iter().map(|field| {
        let ident = &field.ident;
        match &field.state {
            Some(state) => {
                let ty = &field.ty;
                quote!(<#state as ::sycamore::rt::FieldValue<#fields_module::#ident, #ty>>::into_value(self.#ident))
            }
            None if field.default.is_some() => {
                let value = &default_value_names[default_index];
                default_index += 1;
                quote!(#value)
            }
            None => quote!(self.#ident),
        }
    });

    let invocation = if opaque_output {
        quote!((self.#callback_name)(#(#arguments),*))
    } else {
        quote!(#helper #helper_turbofish (#(#arguments),*))
    };

    let (default_helper, default_resolution) = if default_fields.is_empty() {
        (quote!(), quote!())
    } else {
        let root_default_types: Vec<_> = default_fields.iter().map(|field| &field.ty).collect();
        let default_field_names: Vec<_> = default_fields.iter().map(|field| &field.ident).collect();
        let root_predicates = root_where
            .iter()
            .flat_map(|clause| clause.predicates.iter());
        let root_default_bounds = default_fields.iter().filter_map(|field| {
            if matches!(field.default, Some(DefaultValue::Trait)) {
                let ty = &field.ty;
                Some(quote!(#ty: ::core::default::Default))
            } else {
                None
            }
        });
        let helper_bounds: Vec<_> = root_predicates
            .map(|predicate| quote!(#predicate))
            .chain(root_default_bounds)
            .collect();
        let helper_where = if helper_bounds.is_empty() {
            quote!()
        } else {
            quote!(where #(#helper_bounds),*)
        };
        let default_values = default_fields
            .iter()
            .zip(&default_argument_names)
            .zip(&default_value_names)
            .map(|((field, argument), value)| {
                let fallback = match field.default.as_ref().unwrap() {
                    DefaultValue::Trait => quote!(::core::default::Default::default()),
                    DefaultValue::Function(expression) => quote!((#expression)()),
                };
                quote!(match #argument {
                    ::core::option::Option::Some(#value) => #value,
                    ::core::option::Option::None => #fallback,
                })
            });
        (
            quote! {
                #(#shared_attrs)*
                #[allow(clippy::multiple_bound_locations)]
                fn #defaults_helper #root_generics (
                    #(#default_argument_names: ::core::option::Option<#root_default_types>),*
                ) -> (#(#root_default_types,)*) #helper_where {
                    (#(#default_values,)*)
                }
            },
            quote! {
                let (#(#default_value_names,)*) = #defaults_helper #default_turbofish (
                    #(self.#default_field_names),*
                );
            },
        )
    };

    let required_setters = required.iter().map(|field| {
        let attrs = &field.attrs;
        let name = &field.ident;
        let ty = &field.ty;
        let (setter_type, setter_value) = setter_input(field, ty);
        let other_states: Vec<_> = states
            .iter()
            .filter(|state| **state != field.state.as_ref().unwrap())
            .copied()
            .collect();
        let impl_parameters = angle(
            &base_declarations
                .iter()
                .cloned()
                .chain(other_states.iter().map(|state| quote!(#state)))
                .collect::<Vec<_>>(),
        );
        let before: Vec<_> = required
            .iter()
            .map(|f| {
                if f.ident == *name {
                    quote!(::sycamore::rt::Missing)
                } else {
                    let state = f.state.as_ref().unwrap();
                    quote!(#state)
                }
            })
            .collect();
        let after: Vec<_> = required
            .iter()
            .map(|f| {
                if f.ident == *name {
                    quote!(::sycamore::rt::Set<#ty>)
                } else {
                    let state = f.state.as_ref().unwrap();
                    quote!(#state)
                }
            })
            .collect();
        let before = angle(
            &base_arguments
                .iter()
                .cloned()
                .chain(before)
                .collect::<Vec<_>>(),
        );
        let after = angle(
            &base_arguments
                .iter()
                .cloned()
                .chain(after)
                .collect::<Vec<_>>(),
        );
        let values = fields.iter().map(|f| {
            let ident = &f.ident;
            if *ident == *name {
                quote!(#ident: ::sycamore::rt::Set(#setter_value))
            } else {
                quote!(#ident: self.#ident)
            }
        });
        quote! {
            #(#shared_attrs)*
            impl #impl_parameters #builder #before #root_where {
                #(#attrs)*
                pub fn #name(self, #name: #setter_type) -> #builder #after {
                    #builder { #(#values,)* #callback_transfer #marker_name: self.#marker_name }
                }
            }
        }
    });
    let optional_setters = fields.iter().filter_map(|field| {
        let setter_type = if field.default.is_some() {
            Some(&field.ty)
        } else {
            field.optional_inner.as_ref()
        };
        setter_type.map(|ty| {
            let attrs = &field.attrs;
            let name = &field.ident;
            let (setter_type, setter_value) = setter_input(field, ty);
            quote! {
                #(#attrs)*
                pub fn #name(mut self, #name: #setter_type) -> Self {
                    self.#name = ::core::option::Option::Some(#setter_value);
                    self
                }
            }
        })
    });

    let builder = &builder_info.ident;
    let tokens = quote! {
        #(#shared_attrs)*
        #function

        #default_helper

        #(#shared_attrs)*
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #visibility struct #builder #parameters #root_where {
            #(#field_names: #field_types,)*
            #callback_field
            #marker_name: ::core::marker::PhantomData<(#(#marker_elements,)* fn() -> *const (#(#callback_types,)*))>,
        }

        #(#shared_attrs)*
        #[doc(hidden)]
        mod #fields_module { #(#markers)* }

        #(#required_setters)*

        #(#shared_attrs)*
        impl #parameters #builder #type_parameters #root_where {
            #(#optional_setters)*

            pub fn build(self) -> #result_type #where_clause {
                #default_resolution
                #invocation
            }
        }

        #(#shared_attrs)*
        #[allow(clippy::multiple_bound_locations)]
        impl #parameters ::sycamore::rt::Build<#result_type> for #builder #type_parameters #combined_where {
            fn build(self) -> #result_type {
                #builder::build(self)
            }
        }

        #(#attrs)*
        #visibility fn #name #root_generics () -> #builder #root_missing_parameters #root_where {
            #builder { #(#field_names: #initial_values,)* #callback_initial #marker_name: ::core::marker::PhantomData }
        }
    };
    Ok(BuilderExpansion {
        tokens,
        builder: builder_info,
    })
}

struct InputLifetimes {
    used: Vec<Ident>,
    seen: Vec<syn::Lifetime>,
    generated: Vec<syn::Lifetime>,
    bound: Vec<Ident>,
    next: usize,
}

impl InputLifetimes {
    fn new(generics: &syn::Generics) -> Self {
        Self {
            used: generics
                .lifetimes()
                .map(|param| param.lifetime.ident.clone())
                .collect(),
            seen: Vec::new(),
            generated: Vec::new(),
            bound: Vec::new(),
            next: 0,
        }
    }

    fn reserve_names(&mut self, ty: &Type) {
        struct Collector<'a>(&'a mut Vec<Ident>);
        impl<'ast> Visit<'ast> for Collector<'_> {
            fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
                self.0.push(lifetime.ident.clone());
            }
        }
        Collector(&mut self.used).visit_type(ty);
    }

    fn fresh(&mut self) -> syn::Lifetime {
        loop {
            let ident = format_ident!("__builder_lifetime_{}", self.next);
            self.next += 1;
            if !self.used.contains(&ident) {
                self.used.push(ident.clone());
                let lifetime = syn::Lifetime::new(&format!("'{ident}"), ident.span());
                self.generated.push(lifetime.clone());
                return lifetime;
            }
        }
    }
}

impl VisitMut for InputLifetimes {
    fn visit_type_reference_mut(&mut self, node: &mut syn::TypeReference) {
        if node.lifetime.is_none() && self.bound.is_empty() {
            node.lifetime = Some(self.fresh());
        }
        visit_mut::visit_type_reference_mut(self, node);
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if self.bound.contains(&lifetime.ident) {
            return;
        }
        if lifetime.ident == "_" {
            if !self.bound.is_empty() {
                return;
            }
            *lifetime = self.fresh();
        }
        if !self.seen.iter().any(|seen| seen.ident == lifetime.ident) {
            self.seen.push(lifetime.clone());
        }
    }

    fn visit_trait_bound_mut(&mut self, bound: &mut syn::TraitBound) {
        let previous_len = self.bound.len();
        if let Some(lifetimes) = &bound.lifetimes {
            self.bound
                .extend(lifetimes.lifetimes.iter().filter_map(|param| match param {
                    syn::GenericParam::Lifetime(param) => Some(param.lifetime.ident.clone()),
                    _ => None,
                }));
        }
        visit_mut::visit_trait_bound_mut(self, bound);
        self.bound.truncate(previous_len);
    }

    // Lifetimes in function-pointer and Fn-trait signatures have their own
    // higher-ranked elision scope, rather than the enclosing function's scope.
    fn visit_type_fn_ptr_mut(&mut self, _: &mut syn::TypeFnPtr) {}
    fn visit_parenthesized_generic_arguments_mut(
        &mut self,
        _: &mut syn::ParenthesizedGenericArguments,
    ) {
    }
}

struct OutputLifetimes {
    inferred: Option<syn::Lifetime>,
    error: Option<syn::Error>,
}

impl OutputLifetimes {
    fn inferred(&mut self, span: Span) -> Option<syn::Lifetime> {
        if self.inferred.is_none() && self.error.is_none() {
            self.error = Some(syn::Error::new(
                span,
                "`#[builder]` cannot infer the elided output lifetime; use an explicit lifetime",
            ));
        }
        self.inferred.clone()
    }
}

impl VisitMut for OutputLifetimes {
    fn visit_type_reference_mut(&mut self, node: &mut syn::TypeReference) {
        if node.lifetime.is_none() {
            node.lifetime = self.inferred(node.and_token.span);
        }
        visit_mut::visit_type_reference_mut(self, node);
    }

    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_"
            && let Some(inferred) = self.inferred(lifetime.apostrophe)
        {
            *lifetime = inferred;
        }
    }

    fn visit_type_fn_ptr_mut(&mut self, _: &mut syn::TypeFnPtr) {}
    fn visit_parenthesized_generic_arguments_mut(
        &mut self,
        _: &mut syn::ParenthesizedGenericArguments,
    ) {
    }
}

struct ImplArgumentLowerer {
    used: Vec<Ident>,
    params: Vec<syn::GenericParam>,
    next: usize,
}

impl ImplArgumentLowerer {
    fn new(generics: &syn::Generics) -> Self {
        let used = generics
            .params
            .iter()
            .filter_map(|param| match param {
                syn::GenericParam::Type(param) => Some(param.ident.clone()),
                syn::GenericParam::Const(param) => Some(param.ident.clone()),
                syn::GenericParam::Lifetime(_) => None,
            })
            .collect();
        Self {
            used,
            params: Vec::new(),
            next: 0,
        }
    }
}

impl VisitMut for ImplArgumentLowerer {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::ImplTrait(opaque) = ty {
            let ident = loop {
                let ident = format_ident!("__BuilderImpl{}", self.next);
                self.next += 1;
                if !self.used.contains(&ident) {
                    break ident;
                }
            };
            self.used.push(ident.clone());
            let bounds = opaque.bounds.clone();
            self.params.push(parse_quote!(#ident: #bounds));
            *ty = parse_quote!(#ident);
        } else {
            visit_mut::visit_type_mut(self, ty);
        }
    }
}

// Keep the constructor's attributes unchanged. For the other generated items,
// retain only shared attributes, preserving every enclosing cfg_attr condition.
fn shared_attribute_meta(meta: &syn::Meta) -> syn::Result<Option<syn::Meta>> {
    if ["cfg", "allow", "warn", "deny", "forbid"]
        .iter()
        .any(|name| meta.path().is_ident(name))
    {
        return Ok(Some(meta.clone()));
    }
    if !meta.path().is_ident("cfg_attr") {
        return Ok(None);
    }
    let syn::Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(
            meta,
            "expected cfg_attr(condition, attributes...)",
        ));
    };
    let arguments = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
    )?;
    let mut arguments = arguments.into_iter();
    let condition = arguments.next().ok_or_else(|| {
        syn::Error::new_spanned(meta, "expected cfg_attr(condition, attributes...)")
    })?;
    let mut shared = Vec::new();
    let mut has_attributes = false;
    for argument in arguments {
        has_attributes = true;
        if let Some(attribute) = shared_attribute_meta(&argument)? {
            shared.push(attribute);
        }
    }
    if !has_attributes {
        return Err(syn::Error::new_spanned(
            meta,
            "expected at least one attribute after cfg_attr condition",
        ));
    }
    if shared.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_quote!(cfg_attr(#condition, #(#shared),*))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_complete_builder_metadata_for_extension_impls() {
        let functions: Vec<ItemFn> = vec![
            parse_quote!(
                fn empty() {}
            ),
            parse_quote! {
                fn generic<'a, T, const N: usize>(
                    value: &'a [T; N],
                    #[builder(default)] fallback: T,
                ) -> &'a [T; N] where T: Clone { value }
            },
            parse_quote! {
                fn opaque(value: impl core::fmt::Display) -> impl core::fmt::Display { value }
            },
            parse_quote! {
                async fn asynchronous(value: &str, other: impl core::fmt::Display) -> usize {
                    value.len()
                }
            },
            parse_quote! {
                fn collisions<__BuilderField0, __BuilderOutput>(
                    first: __BuilderField0, second: __BuilderOutput,
                ) -> impl Sized { (first, second) }
            },
        ];
        // Original generics, inferred input types/lifetimes, opaque outputs, and field states.
        let expected_parameters = [0, 4, 3, 5, 5];
        for (function, expected_parameters) in functions.into_iter().zip(expected_parameters) {
            let expansion = expand(function).unwrap();
            let file: syn::File = syn::parse2(expansion.tokens).unwrap();
            let info = expansion.builder;
            let generated = file
                .items
                .iter()
                .find_map(|item| match item {
                    syn::Item::Struct(item) if item.ident == info.ident => Some(item),
                    _ => None,
                })
                .expect("metadata must identify the generated builder");
            let actual = &info.generics;
            let expected = &generated.generics;
            assert_eq!(quote!(#actual).to_string(), quote!(#expected).to_string());
            let actual_where = &actual.where_clause;
            let expected_where = &expected.where_clause;
            assert_eq!(
                quote!(#actual_where).to_string(),
                quote!(#expected_where).to_string()
            );
            assert_eq!(actual.params.len(), expected_parameters);

            let ident = &info.ident;
            let (impl_generics, ty_generics, where_clause) = info.generics.split_for_impl();
            let extension: syn::ItemImpl = syn::parse2(quote! {
                impl #impl_generics BackendTrait for #ident #ty_generics #where_clause {}
            })
            .unwrap();
            assert_eq!(extension.generics.params.len(), expected_parameters);
            let bounds = quote!(#where_clause).to_string();
            assert!(
                !bounds.contains("FieldValue"),
                "extensions must apply to incomplete builders"
            );
            assert!(
                !bounds.contains("Default"),
                "extensions must not inherit build-only bounds"
            );
        }
    }

    #[test]
    fn forwards_shared_attributes_to_all_top_level_items() {
        let function = parse_quote! {
            #[cfg(feature = "example")]
            #[cfg_attr(feature = "example", allow(dead_code))]
            #[allow(dead_code)]
            #[expect(unused_variables)]
            #[warn(unused_mut)]
            #[deny(unreachable_code)]
            #[forbid(unsafe_code)]
            #[doc = "Constructor documentation"]
            #[must_use]
            #[custom::attribute(arbitrary)]
            fn example(#[builder(default)] value: i32) -> i32 { value }
        };
        let expanded: syn::File = syn::parse2(expand(function).unwrap().tokens).unwrap();
        assert_eq!(expanded.items.len(), 7);
        for (index, item) in expanded.items.iter().enumerate() {
            let attrs = match item {
                syn::Item::Fn(item) => &item.attrs,
                syn::Item::Struct(item) => &item.attrs,
                syn::Item::Impl(item) => &item.attrs,
                syn::Item::Mod(item) => &item.attrs,
                _ => panic!("unexpected generated item"),
            };
            for name in ["cfg", "cfg_attr", "allow", "warn", "deny", "forbid"] {
                assert!(attrs.iter().any(|attr| attr.path().is_ident(name)));
            }
            let is_constructor = index == expanded.items.len() - 1;
            assert_eq!(
                attrs.iter().any(|attr| {
                    let meta = &attr.meta;
                    quote!(#meta).to_string() == "doc = \"Constructor documentation\""
                }),
                is_constructor
            );
            for name in ["must_use", "expect"] {
                assert_eq!(
                    attrs.iter().any(|attr| attr.path().is_ident(name)),
                    is_constructor,
                );
            }
            assert_eq!(
                attrs
                    .iter()
                    .any(|attr| attr.path().segments.first().unwrap().ident == "custom"),
                is_constructor,
            );
        }
    }

    #[test]
    fn forwards_parameter_attributes_only_to_setter_methods() {
        let function: ItemFn = parse_quote! {
            fn example(
                /// Required value.
                #[must_use]
                #[custom::attribute(arbitrary)]
                #[builder(into)]
                required: String,
                /// Optional value.
                #[cfg_attr(unix, deprecated(note = "use another setter"))]
                optional: Option<i32>,
                /// Defaulted value.
                #[inline]
                #[builder(default)]
                defaulted: i32,
                /// Explicitly required option.
                #[builder(required)]
                explicit: Option<i32>,
            ) {}
        };
        let expected: Vec<_> = function
            .sig
            .inputs
            .iter()
            .map(|arg| {
                let FnArg::Typed(arg) = arg else {
                    panic!("expected typed argument")
                };
                let attrs: Vec<_> = arg
                    .attrs
                    .iter()
                    .filter(|attr| !attr.path().is_ident("builder"))
                    .collect();
                (quote!(#(#attrs)*).to_string(), false)
            })
            .collect();
        let expanded: syn::File = syn::parse2(expand(function).unwrap().tokens).unwrap();
        let mut expected = expected;
        for item in &expanded.items {
            match item {
                syn::Item::Fn(function) => {
                    for arg in &function.sig.inputs {
                        let FnArg::Typed(arg) = arg else {
                            panic!("expected typed argument")
                        };
                        assert!(arg.attrs.is_empty());
                    }
                }
                syn::Item::Impl(implementation) => {
                    for item in &implementation.items {
                        let syn::ImplItem::Fn(method) = item else {
                            continue;
                        };
                        let index = match method.sig.ident.to_string().as_str() {
                            "required" => 0,
                            "optional" => 1,
                            "defaulted" => 2,
                            "explicit" => 3,
                            _ => continue,
                        };
                        let attrs = &method.attrs;
                        assert_eq!(quote!(#(#attrs)*).to_string(), expected[index].0);
                        assert!(!expected[index].1, "duplicate setter");
                        expected[index].1 = true;
                    }
                }
                syn::Item::Struct(_) | syn::Item::Mod(_) => {}
                _ => panic!("unexpected generated item"),
            }
        }
        assert!(expected.iter().all(|(_, seen)| *seen));
    }

    #[test]
    fn isolates_only_field_markers_in_a_private_module() {
        let function = parse_quote! {
            fn example(first: LocalType, second: LocalType, optional: Option<LocalType>) {}
        };
        let expanded: syn::File = syn::parse2(expand(function).unwrap().tokens).unwrap();
        let modules: Vec<_> = expanded
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Mod(module) = item {
                    Some(module)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(modules.len(), 1);
        let module = modules[0];
        assert_eq!(module.ident, "__builder_example_fields");
        assert!(matches!(module.vis, Visibility::Inherited));
        let items = &module.content.as_ref().unwrap().1;
        assert_eq!(items.len(), 2);
        for (item, name) in items.iter().zip(["first", "second"]) {
            let syn::Item::Struct(marker) = item else {
                panic!("expected field marker")
            };
            assert_eq!(marker.ident, name);
            assert!(matches!(marker.vis, Visibility::Public(_)));
            assert!(matches!(marker.fields, syn::Fields::Unit));
            assert!(marker.generics.params.is_empty());
        }
    }

    #[test]
    fn filters_cfg_attr_recursively() {
        let original: syn::Meta = parse_quote! {
            cfg_attr(feature = "outer",
                must_use,
                allow(dead_code),
                expect(unused_variables),
                cfg_attr(feature = "inner", cfg(unix), forbid(unsafe_code), inline),
                cfg_attr(feature = "constructor_only", deprecated)
            )
        };
        let expected: syn::Meta = parse_quote! {
            cfg_attr(feature = "outer",
                allow(dead_code),
                cfg_attr(feature = "inner", cfg(unix), forbid(unsafe_code))
            )
        };
        let actual = shared_attribute_meta(&original).unwrap().unwrap();
        assert_eq!(quote!(#actual).to_string(), quote!(#expected).to_string());

        let constructor_only: syn::Meta = parse_quote! {
            cfg_attr(feature = "outer", expect(dead_code), cfg_attr(unix, must_use))
        };
        assert!(shared_attribute_meta(&constructor_only).unwrap().is_none());
    }

    #[test]
    fn preserves_original_cfg_attr_only_on_constructor() {
        let function = parse_quote! {
            #[cfg_attr(feature = "example", cfg(unix), must_use, expect(dead_code))]
            fn example() {}
        };
        let expanded: syn::File = syn::parse2(expand(function).unwrap().tokens).unwrap();
        let shared: syn::Meta = parse_quote!(cfg_attr(feature = "example", cfg(unix)));
        let original: syn::Meta = parse_quote! {
            cfg_attr(feature = "example", cfg(unix), must_use, expect(dead_code))
        };
        for (index, item) in expanded.items.iter().enumerate() {
            let attrs = match item {
                syn::Item::Fn(item) => &item.attrs,
                syn::Item::Struct(item) => &item.attrs,
                syn::Item::Impl(item) => &item.attrs,
                syn::Item::Mod(item) => &item.attrs,
                _ => panic!("unexpected generated item"),
            };
            let actual = &attrs
                .iter()
                .find(|attr| attr.path().is_ident("cfg_attr"))
                .unwrap()
                .meta;
            let expected = if index == expanded.items.len() - 1 {
                &original
            } else {
                &shared
            };
            assert_eq!(quote!(#actual).to_string(), quote!(#expected).to_string());
        }
    }

    #[test]
    fn forwards_shared_attributes_without_a_defaults_helper() {
        let function = parse_quote! {
            #[cfg_attr(feature = "example", allow(dead_code))]
            fn example(value: i32) -> i32 { value }
        };
        let expanded: syn::File = syn::parse2(expand(function).unwrap().tokens).unwrap();
        assert_eq!(expanded.items.len(), 7);
        for item in &expanded.items {
            let attrs = match item {
                syn::Item::Fn(item) => &item.attrs,
                syn::Item::Struct(item) => &item.attrs,
                syn::Item::Impl(item) => &item.attrs,
                syn::Item::Mod(item) => &item.attrs,
                _ => panic!("unexpected generated item"),
            };
            assert!(attrs.iter().any(|attr| attr.path().is_ident("cfg_attr")));
        }
    }
}

fn angle<T: quote::ToTokens>(items: &[T]) -> TokenStream2 {
    if items.is_empty() {
        quote!()
    } else {
        quote!(<#(#items),*>)
    }
}

fn setter_input(field: &Field, ty: &Type) -> (TokenStream2, TokenStream2) {
    let name = &field.ident;
    if field.into {
        (
            quote!(impl ::core::convert::Into<#ty>),
            quote!(<_ as ::core::convert::Into<#ty>>::into(#name)),
        )
    } else {
        (quote!(#ty), quote!(#name))
    }
}

fn parameter_options(attrs: &[syn::Attribute]) -> syn::Result<ParameterOptions> {
    let mut options = ParameterOptions::default();
    for attr in attrs {
        if !attr.path().is_ident("builder") {
            continue;
        }
        let mut found = false;
        attr.parse_nested_meta(|meta| {
            found = true;
            if meta.path.is_ident("required") {
                if options.required {
                    return Err(meta.error("duplicate `#[builder(required)]`"));
                }
                options.required = true;
            } else if meta.path.is_ident("default") {
                if options.default.is_some() {
                    return Err(meta.error("duplicate `#[builder(default)]`"));
                }
                options.default = Some(if meta.input.peek(syn::Token![=]) {
                    DefaultValue::Function(meta.value()?.parse()?)
                } else {
                    DefaultValue::Trait
                });
            } else if meta.path.is_ident("into") {
                if options.into {
                    return Err(meta.error("duplicate `#[builder(into)]`"));
                }
                options.into = true;
            } else {
                return Err(meta.error("expected `required`, `default`, or `into`"));
            }
            Ok(())
        })?;
        if !found {
            return Err(syn::Error::new_spanned(
                attr,
                "expected `#[builder(required)]`, `#[builder(default)]`, or `#[builder(into)]`",
            ));
        }
    }
    Ok(options)
}

fn option_inner(ty: &Type) -> Option<Type> {
    let Type::Path(path) = ty else { return None };
    if path.qself.is_some() {
        return None;
    }
    let names: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    if !matches!(names.as_slice(), [name] if name == "Option")
        && !matches!(names.as_slice(), [root, module, name] if (root == "std" || root == "core") && module == "option" && name == "Option")
    {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &path.path.segments.last()?.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        GenericArgument::Type(ty) => Some(ty.clone()),
        _ => None,
    }
}

fn contains_impl_trait(ty: &Type) -> bool {
    struct Finder(bool);
    impl<'ast> Visit<'ast> for Finder {
        fn visit_type_impl_trait(&mut self, _: &'ast syn::TypeImplTrait) {
            self.0 = true;
        }
    }
    let mut finder = Finder(false);
    finder.visit_type(ty);
    finder.0
}

fn validate_type(ty: &Type, allow_impl_trait: bool) -> syn::Result<()> {
    struct Check(Option<syn::Error>, bool);
    impl<'ast> Visit<'ast> for Check {
        fn visit_type_impl_trait(&mut self, node: &'ast syn::TypeImplTrait) {
            if !self.1 {
                self.0.get_or_insert_with(|| {
                    syn::Error::new_spanned(
                        node,
                        "`#[builder]` could not lower this `impl Trait` argument",
                    )
                });
            }
            visit::visit_type_impl_trait(self, node);
        }
        fn visit_type_infer(&mut self, node: &'ast syn::TypeInfer) {
            self.0.get_or_insert_with(|| {
                syn::Error::new_spanned(node, "`#[builder]` requires explicit types")
            });
        }
        fn visit_type_reference(&mut self, node: &'ast syn::TypeReference) {
            if node
                .lifetime
                .as_ref()
                .is_none_or(|lifetime| lifetime.ident == "_")
            {
                self.0.get_or_insert_with(|| {
                    syn::Error::new_spanned(
                        node,
                        "`#[builder]` requires an explicit lifetime on references",
                    )
                });
            }
            visit::visit_type_reference(self, node);
        }
        fn visit_type_never(&mut self, node: &'ast syn::TypeNever) {
            self.0.get_or_insert_with(|| {
                syn::Error::new_spanned(node, "`#[builder]` does not support the never type")
            });
        }
        fn visit_type_fn_ptr(&mut self, _: &'ast syn::TypeFnPtr) {}
        fn visit_parenthesized_generic_arguments(
            &mut self,
            _: &'ast syn::ParenthesizedGenericArguments,
        ) {
        }
    }
    let mut check = Check(None, allow_impl_trait);
    check.visit_type(ty);
    match check.0 {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
