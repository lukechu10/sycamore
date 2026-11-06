//! Automatically convert function arguments into a fluent API for Sycamore components.
//!
//! It supports synchronous, crate-local free functions with named arguments, generic parameters,
//! and argument- and return-position `impl Trait`. `Option<T>`, `std::option::Option<T>`, and
//! `core::option::Option<T>` arguments are optional; aliases are not recognized and an unqualified
//! `Option` must refer to the standard type.
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
//! Argument-position `impl Trait` introduces an inferred constructor type parameter; callers
//! specifying explicit type arguments may need an additional `_`. An unset `Option<impl Trait>` may
//! require an explicit type argument. Opaque return types use a function pointer in the builder and
//! still implement `Build<T>`.
//!
//! Expansion creates a module named after the function and a private helper named
//! `__builder_<function>_inner`. A function with defaults also gets a
//! `__builder_<function>_defaults` helper. These names must not collide with existing items.
//! The original function name becomes the builder constructor, so recursive calls to that name in
//! the original body are not supported.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::visit::{self, Visit};
use syn::visit_mut::{self, VisitMut};
use syn::{
    FnArg, GenericArgument, Ident, ItemFn, Pat, PathArguments, ReturnType, Type, Visibility,
    parse_quote,
};

struct Field {
    ident: Ident,
    root_ty: Type,
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

pub fn expand(mut function: ItemFn) -> syn::Result<TokenStream2> {
    let signature = &function.sig;
    if signature.constness.is_some()
        || signature.asyncness.is_some()
        || !matches!(signature.safety, syn::Safety::Default)
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(syn::Error::new_spanned(
            signature,
            "`builder` supports only synchronous, safe, non-const Rust functions",
        ));
    }
    if matches!(function.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &function.vis,
            "`builder` currently supports crate-local functions only",
        ));
    }
    for attr in &function.attrs {
        if !attr.path().is_ident("doc") && !attr.path().is_ident("cfg") {
            return Err(syn::Error::new_spanned(
                attr,
                "`builder` currently supports only `doc` and `cfg` attributes",
            ));
        }
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
                "`builder` does not support attributes on generic parameters",
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
    let mut module_generics = root_generics.clone();
    Rebase {
        generic_names: &generic_names,
    }
    .visit_generics_mut(&mut module_generics);
    let mut fields = Vec::new();
    let mut required_count = 0usize;
    for arg in &lowered_inputs {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "`builder` requires a free function",
            ));
        };
        let options = parameter_options(&arg.attrs)?;
        let Pat::Ident(pat) = arg.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "`builder` requires named parameters",
            ));
        };
        if pat.by_ref.is_some() || pat.subpat.is_some() {
            return Err(syn::Error::new_spanned(
                pat,
                "`builder` requires plain named parameters",
            ));
        }
        if pat.ident == "build" {
            return Err(syn::Error::new_spanned(
                &pat.ident,
                "a builder parameter cannot be named `build`",
            ));
        }
        validate_type(&arg.ty, false)?;
        let root_ty = (*arg.ty).clone();
        let mut ty = root_ty.clone();
        rebase_type(&mut ty, &generic_names);
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
            ident: pat.ident.clone(),
            root_ty,
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
    let opaque_output = contains_impl_trait(&root_output);
    let mut output = root_output.clone();
    rebase_type(&mut output, &generic_names);
    let mut output_ident = format_ident!("__BuilderOutput");
    while generic_names.contains(&output_ident) {
        output_ident = format_ident!("_{}", output_ident);
    }

    let name = signature.ident.clone();
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
    let cfg_attrs: Vec<_> = attrs.iter().filter(|a| a.path().is_ident("cfg")).collect();
    function.attrs.clear();
    function.vis = Visibility::Inherited;
    function.sig.ident = helper.clone();
    for arg in &mut function.sig.inputs {
        if let FnArg::Typed(arg) = arg {
            arg.attrs.retain(|attr| !attr.path().is_ident("builder"));
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
    let mut base_declarations: Vec<_> = module_generics
        .params
        .iter()
        .map(|param| quote!(#param))
        .collect();
    let mut base_arguments: Vec<_> = module_generics
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
    let mut new_generics = module_generics.clone();
    if opaque_output {
        base_declarations.push(quote!(#output_ident));
        base_arguments.push(quote!(#output_ident));
        new_generics.params.push(parse_quote!(#output_ident));
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
    let mut constructor_arguments: Vec<_> = root_generics
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
    if opaque_output {
        constructor_arguments.push(quote!(_));
    }
    let constructor_turbofish = if constructor_arguments.is_empty() {
        quote!()
    } else {
        quote!(::<#(#constructor_arguments),*>)
    };
    let parameters = angle(
        &base_declarations
            .iter()
            .cloned()
            .chain(states.iter().map(|state| quote!(#state)))
            .collect::<Vec<_>>(),
    );
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
    let missing_parameters = angle(
        &base_arguments
            .iter()
            .cloned()
            .chain(missing.iter().cloned())
            .collect::<Vec<_>>(),
    );
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
        quote!(#output)
    };
    let module_where = &module_generics.where_clause;
    let root_where = &root_generics.where_clause;
    let original_predicates: Vec<_> = module_where
        .iter()
        .flat_map(|clause| clause.predicates.iter())
        .collect();
    let marker_elements: Vec<_> = module_generics
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
    let root_callback_types: Vec<_> = fields.iter().map(|field| &field.root_ty).collect();
    let callback_field = if opaque_output {
        quote!(#callback_name: fn(#(#callback_types),*) -> #output_ident,)
    } else {
        quote!()
    };
    let callback_parameter = if opaque_output {
        quote!(#callback_name: fn(#(#callback_types),*) -> #output_ident)
    } else {
        quote!()
    };
    let callback_transfer = if opaque_output {
        quote!(#callback_name: self.#callback_name,)
    } else {
        quote!()
    };
    let callback_initial = if opaque_output {
        quote!(#callback_name,)
    } else {
        quote!()
    };
    let constructor_callback = if opaque_output {
        quote!(#helper #helper_turbofish as fn(#(#root_callback_types),*) -> _)
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
        quote!(#[allow(non_camel_case_types)] pub struct #ident;)
    });
    let mut bounds: Vec<_> = required
        .iter()
        .map(|field| {
            let state = field.state.as_ref().unwrap();
            let ident = &field.ident;
            let ty = &field.ty;
            quote!(#state: ::sycamore::rt::FieldValue<self::fields::#ident, #ty>)
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
                quote!(<#state as ::sycamore::rt::FieldValue<self::fields::#ident, #ty>>::into_value(self.#ident))
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
        quote!(super::#helper #helper_turbofish (#(#arguments),*))
    };

    let (default_helper, default_resolution) = if default_fields.is_empty() {
        (quote!(), quote!())
    } else {
        let root_default_types: Vec<_> =
            default_fields.iter().map(|field| &field.root_ty).collect();
        let default_field_names: Vec<_> = default_fields.iter().map(|field| &field.ident).collect();
        let root_predicates = root_where
            .iter()
            .flat_map(|clause| clause.predicates.iter());
        let root_default_bounds = default_fields.iter().filter_map(|field| {
            if matches!(field.default, Some(DefaultValue::Trait)) {
                let ty = &field.root_ty;
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
                #(#cfg_attrs)*
                #[allow(clippy::multiple_bound_locations)]
                fn #defaults_helper #root_generics (
                    #(#default_argument_names: ::core::option::Option<#root_default_types>),*
                ) -> (#(#root_default_types,)*) #helper_where {
                    (#(#default_values,)*)
                }
            },
            quote! {
                let (#(#default_value_names,)*) = super::#defaults_helper #default_turbofish (
                    #(self.#default_field_names),*
                );
            },
        )
    };

    let required_setters = required.iter().map(|field| {
        let name = &field.ident;
        let ty = &field.ty;
        let (setter_type, setter_value) = setter_input(field, ty);
        let other_states: Vec<_> = states
            .iter()
            .filter(|state| **state != field.state.as_ref().unwrap())
            .copied()
            .collect();
        let impl_parameters = angle(&base_declarations.iter().cloned().chain(other_states.iter().map(|state| quote!(#state))).collect::<Vec<_>>());
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
        let before = angle(&base_arguments.iter().cloned().chain(before).collect::<Vec<_>>());
        let after = angle(&base_arguments.iter().cloned().chain(after).collect::<Vec<_>>());
        let values = fields.iter().map(|f| {
            let ident = &f.ident;
            if *ident == *name {
                quote!(#ident: ::sycamore::rt::Set(#setter_value))
            } else {
                quote!(#ident: self.#ident)
            }
        });
        quote! {
            impl #impl_parameters self::Builder #before #module_where {
                pub fn #name(self, #name: #setter_type) -> self::Builder #after {
                    self::Builder { #(#values,)* #callback_transfer #marker_name: self.#marker_name }
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
            let name = &field.ident;
            let (setter_type, setter_value) = setter_input(field, ty);
            quote! {
                pub fn #name(mut self, #name: #setter_type) -> Self {
                    self.#name = ::core::option::Option::Some(#setter_value);
                    self
                }
            }
        })
    });

    Ok(quote! {
        #(#cfg_attrs)*
        #function

        #default_helper

        #(#cfg_attrs)*
        #[allow(unused_imports)]
        #visibility mod #name {
            use super::*;

            mod fields { #(#markers)* }

            pub struct Builder #parameters #module_where {
                #(#field_names: #field_types,)*
                #callback_field
                #marker_name: ::core::marker::PhantomData<(#(#marker_elements,)* fn() -> *const (#(#callback_types,)*))>,
            }

            #(#required_setters)*

            impl #parameters self::Builder #type_parameters #module_where {
                #(#optional_setters)*

                pub fn build(self) -> #result_type #where_clause {
                    #default_resolution
                    #invocation
                }
            }

            #[allow(clippy::multiple_bound_locations)]
            impl #parameters ::sycamore::rt::Build<#result_type> for self::Builder #type_parameters #combined_where {
                fn build(self) -> #result_type {
                    self::Builder::build(self)
                }
            }

            pub fn new #new_generics (#callback_parameter) -> self::Builder #missing_parameters #module_where {
                self::Builder { #(#field_names: #initial_values,)* #callback_initial #marker_name: ::core::marker::PhantomData }
            }
        }

        #(#attrs)*
        #visibility fn #name #root_generics () -> #name::Builder #root_missing_parameters #root_where {
            #name::new #constructor_turbofish (#constructor_callback)
        }
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
                "`builder` cannot infer the elided output lifetime; use an explicit lifetime",
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
            return Err(syn::Error::new_spanned(
                attr,
                "`builder` does not support this parameter attribute",
            ));
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

// Types are copied into a child module. Preserve relative paths and avoid
// shadowing caller types with generated items and state parameters.
struct Rebase<'a> {
    generic_names: &'a [Ident],
}

impl VisitMut for Rebase<'_> {
    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        visit_mut::visit_path_mut(self, path);
        if path.leading_colon.is_none()
            && let Some(first) = path.segments.first_mut()
        {
            if first.ident == "self" {
                first.ident = Ident::new("super", first.ident.span());
            } else if !self.generic_names.contains(&first.ident)
                && (first.ident == "super"
                    || first.ident == "Builder"
                    || first.ident == "fields"
                    || first.ident.to_string().starts_with("__BuilderField"))
            {
                path.segments.insert(0, parse_quote!(super));
            }
        }
    }
}

fn rebase_type(ty: &mut Type, generic_names: &[Ident]) {
    Rebase { generic_names }.visit_type_mut(ty);
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
                        "`builder` could not lower this `impl Trait` argument",
                    )
                });
            }
            visit::visit_type_impl_trait(self, node);
        }
        fn visit_type_infer(&mut self, node: &'ast syn::TypeInfer) {
            self.0.get_or_insert_with(|| {
                syn::Error::new_spanned(node, "`builder` requires explicit types")
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
                        "`builder` requires an explicit lifetime on references",
                    )
                });
            }
            visit::visit_type_reference(self, node);
        }
        fn visit_type_never(&mut self, node: &'ast syn::TypeNever) {
            self.0.get_or_insert_with(|| {
                syn::Error::new_spanned(node, "`builder` does not support the never type")
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
