//! The `Props` derive macro implementation.
//!
//! _Credits: This code is mostly taken from <https://github.com/idanarye/rust-typed-builder>_

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{DeriveInput, Error, Result};

pub fn impl_derive_props(ast: &DeriveInput) -> Result<TokenStream> {
    let data = match &ast.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => {
                let struct_info = struct_info::StructInfo::new(ast, fields.named.iter())?;
                let builder_creation = struct_info.builder_creation_impl()?;
                let conversion_helper = struct_info.conversion_helper_impl()?;
                let fields = struct_info
                    .included_fields()
                    .map(|f| struct_info.field_impl(f))
                    .collect::<Result<Vec<_>>>()?;
                let fields = quote!(#(#fields)*).into_iter();
                let required_fields = struct_info
                    .included_fields()
                    .filter(|f| f.builder_attr.default.is_none())
                    .map(|f| struct_info.required_field_impl(f))
                    .collect::<Result<Vec<_>>>()?;
                let build_method = struct_info.build_method_impl();

                quote! {
                    #builder_creation
                    #conversion_helper
                    #( #fields )*
                    #( #required_fields )*
                    #build_method
                }
            }
            syn::Fields::Unnamed(_) => {
                return Err(Error::new(
                    ast.span(),
                    "Props is not supported for tuple structs",
                ))
            }
            syn::Fields::Unit => {
                return Err(Error::new(
                    ast.span(),
                    "Props is not supported for unit structs",
                ))
            }
        },
        syn::Data::Enum(_) => {
            return Err(Error::new(ast.span(), "Props is not supported for enums"))
        }
        syn::Data::Union(_) => {
            return Err(Error::new(ast.span(), "Props is not supported for unions"))
        }
    };
    Ok(data)
}

mod struct_info {
    use std::fmt::Write;

    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::parse::Error;
    use syn::punctuated::Punctuated;
    use syn::Token;

    use super::field_info::{AttributeBase, FieldBuilderAttr, FieldInfo};
    use super::util::{
        empty_type, empty_type_tuple, expr_to_single_string, make_punctuated_single,
        modify_types_generics_hack, path_to_single_string, strip_raw_ident_prefix, type_tuple,
    };

    #[derive(Debug)]
    pub struct StructInfo<'a> {
        pub vis: &'a syn::Visibility,
        pub name: &'a syn::Ident,
        pub generics: &'a syn::Generics,
        pub fields: Vec<FieldInfo<'a>>,

        pub builder_attr: TypeBuilderAttr,
        pub builder_name: syn::Ident,
        pub conversion_helper_trait_name: syn::Ident,
        #[allow(dead_code)] // TODO: remove this field?
        pub core: syn::Ident,

        pub attributes: Option<(AttributeBase, String)>,
    }

    impl<'a> StructInfo<'a> {
        pub fn included_fields(&self) -> impl Iterator<Item = &FieldInfo<'a>> {
            self.fields
                .iter()
                .filter(|f| f.builder_attr.setter.skip.is_none())
        }

        pub fn new(
            ast: &'a syn::DeriveInput,
            fields: impl Iterator<Item = &'a syn::Field>,
        ) -> Result<StructInfo<'a>, Error> {
            let builder_attr = TypeBuilderAttr::new(&ast.attrs)?;
            let builder_name = strip_raw_ident_prefix(format!("{}Builder", ast.ident));
            let mut fields = fields
                .enumerate()
                .map(|(i, f)| FieldInfo::new(i, f, builder_attr.field_defaults.clone()))
                .collect::<Result<Vec<FieldInfo>, _>>()?;

            // Search `fields` for `attributes`. If one is found, make sure that it is the only
            // one.
            let mut attributes = None;
            for field in &fields {
                if let Some((base, tag)) = &field.builder_attr.attributes {
                    if attributes.is_some() {
                        return Err(Error::new(
                            field.name.span(),
                            "Only one field can have `#[prop(attributes(...))]`",
                        ));
                    };
                    if field.name != "attributes" {
                        return Err(Error::new(
                            field.name.span(),
                            "The field with `#[prop(attributes(...))]` must be named `attributes`",
                        ));
                    }
                    attributes = Some((base.clone(), tag.clone()));
                }
            }
            // Now filter out `attributes` from `fields`.
            fields.retain(|f| f.builder_attr.attributes.is_none());

            Ok(StructInfo {
                vis: &ast.vis,
                name: &ast.ident,
                generics: &ast.generics,
                fields,
                builder_attr,
                builder_name: syn::Ident::new(&builder_name, proc_macro2::Span::call_site()),
                conversion_helper_trait_name: syn::Ident::new(
                    &format!("{builder_name}_Optional"),
                    proc_macro2::Span::call_site(),
                ),
                core: syn::Ident::new(
                    &format!("{builder_name}_core"),
                    proc_macro2::Span::call_site(),
                ),
                attributes,
            })
        }

        fn modify_generics<F: FnMut(&mut syn::Generics)>(&self, mut mutator: F) -> syn::Generics {
            let mut generics = self.generics.clone();
            mutator(&mut generics);
            generics
        }

        pub fn builder_creation_impl(&self) -> Result<TokenStream, Error> {
            let StructInfo {
                ref vis,
                ref name,
                ref builder_name,
                ..
            } = self;
            let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
            let all_fields_param = syn::GenericParam::Type(
                syn::Ident::new("PropsFields", proc_macro2::Span::call_site()).into(),
            );
            let b_generics = self.modify_generics(|g| {
                g.params.insert(0, all_fields_param.clone());
            });
            let empties_tuple = type_tuple(self.included_fields().map(|_| empty_type()));
            let generics_with_empty = modify_types_generics_hack(&ty_generics, |args| {
                args.insert(0, syn::GenericArgument::Type(empties_tuple.clone().into()));
            });
            let phantom_generics = self.generics.params.iter().map(|param| match param {
                syn::GenericParam::Lifetime(lifetime) => {
                    let lifetime = &lifetime.lifetime;
                    quote!(::core::marker::PhantomData<&#lifetime ()>)
                }
                syn::GenericParam::Type(ty) => {
                    let ty = &ty.ident;
                    quote!(::core::marker::PhantomData<#ty>)
                }
                syn::GenericParam::Const(_cnst) => {
                    quote!()
                }
            });
            let builder_method_doc = match self.builder_attr.builder_method_doc {
                Some(ref doc) => quote!(#doc),
                None => {
                    let doc = format!(
                        "
                    Create a builder for building `{name}`.
                    On the builder, call {setters} to set the values of the fields.
                    Finally, call `.build()` to create the instance of `{name}`.
                    ",
                        name = self.name,
                        setters = {
                            let mut result = String::new();
                            let mut is_first = true;
                            for field in self.included_fields() {
                                use std::fmt::Write;
                                if is_first {
                                    is_first = false;
                                } else {
                                    write!(&mut result, ", ").unwrap();
                                }
                                write!(&mut result, "`.{}(...)`", field.name).unwrap();
                                if field.builder_attr.default.is_some() {
                                    write!(&mut result, "(optional)").unwrap();
                                }
                            }
                            result
                        }
                    );
                    quote!(#doc)
                }
            };
            let builder_type_doc = if self.builder_attr.doc {
                match self.builder_attr.builder_type_doc {
                    Some(ref doc) => quote!(#[doc = #doc]),
                    None => {
                        let doc = format!(
                        "Builder for [`{name}`] instances.\n\nSee [`{name}::builder()`] for more info."
                    );
                        quote!(#[doc = #doc])
                    }
                }
            } else {
                quote!(#[doc(hidden)])
            };

            let (b_generics_impl, b_generics_ty, b_generics_where) = b_generics.split_for_impl();

            let attributes_field = if self.attributes.is_some() {
                quote! { attributes: ::sycamore::web::Attributes, }
            } else {
                quote! { attributes: (), }
            };
            // Check if we need to generate impls for attributes.
            let attributes_impl = if let Some((base, tag)) = &self.attributes {
                let base_trait_ident = match base {
                    AttributeBase::Html => quote!(HtmlGlobalAttributes),
                    AttributeBase::Svg => quote!(SvgGlobalAttributes),
                };
                let tag_camel = tag.split('_').fold(String::new(), |mut acc, segment| {
                    let (first, rest) = segment.split_at(1);
                    write!(&mut acc, "{first}{rest}", first = first.to_uppercase()).unwrap();
                    acc
                });
                let tag_trait_ident = match base {
                    AttributeBase::Html => quote::format_ident!("Html{tag_camel}Attributes"),
                    AttributeBase::Svg => quote::format_ident!("Svg{tag_camel}Attributes"),
                };
                quote! {
                    impl #b_generics_impl ::sycamore::web::GlobalAttributes for #builder_name #b_generics_ty #b_generics_where {}
                    impl #b_generics_impl ::sycamore::web::#base_trait_ident for #builder_name #b_generics_ty #b_generics_where {}
                    impl #b_generics_impl ::sycamore::web::tags::#tag_trait_ident for #builder_name #b_generics_ty #b_generics_where {}

                    impl #b_generics_impl ::sycamore::web::SetAttribute for #builder_name #b_generics_ty #b_generics_where {
                        fn set_attribute(&mut self, name: &'static ::std::primitive::str, value: impl ::sycamore::web::AttributeValue) {
                            self.attributes.set_attribute(name, value);
                        }
                        fn set_event_handler(
                            &mut self,
                            name: &'static ::std::primitive::str,
                            handler: impl ::std::ops::FnMut(::sycamore::rt::Event) + 'static,
                        ) {
                            self.attributes.set_event_handler(name, handler);
                        }
                    }
                }
            } else {
                quote! {}
            };

            Ok(quote! {
                impl #impl_generics ::sycamore::rt::Props for #name #ty_generics #where_clause {
                    type Builder = #builder_name #generics_with_empty;
                    #[doc = #builder_method_doc]
                    #[allow(dead_code, clippy::default_trait_access)]
                    fn builder() -> Self::Builder {
                        #builder_name {
                            fields: #empties_tuple,
                            phantom: ::core::default::Default::default(),
                            attributes: ::core::default::Default::default(),
                        }
                    }
                }

                #[must_use]
                #builder_type_doc
                #[allow(dead_code, non_camel_case_types, non_snake_case)]
                #vis struct #builder_name #b_generics {
                    fields: #all_fields_param,
                    phantom: (#( #phantom_generics ),*),
                    #attributes_field
                }

                #attributes_impl
            })
        }

        // TODO: once the proc-macro crate limitation is lifted, make this an util trait of this
        // crate.
        pub fn conversion_helper_impl(&self) -> Result<TokenStream, Error> {
            let trait_name = &self.conversion_helper_trait_name;
            Ok(quote! {
                #[doc(hidden)]
                #[allow(dead_code, non_camel_case_types, non_snake_case)]
                pub trait #trait_name<T> {
                    fn into_value<F: FnOnce() -> T>(self, default: F) -> T;
                }

                impl<T> #trait_name<T> for () {
                    fn into_value<F: FnOnce() -> T>(self, default: F) -> T {
                        default()
                    }
                }

                impl<T> #trait_name<T> for (T,) {
                    fn into_value<F: FnOnce() -> T>(self, _: F) -> T {
                        self.0
                    }
                }
            })
        }

        pub fn field_impl(&self, field: &FieldInfo) -> Result<TokenStream, Error> {
            let StructInfo {
                ref builder_name, ..
            } = self;

            let destructuring = self.included_fields().map(|f| {
                if f.ordinal == field.ordinal {
                    quote!(_)
                } else {
                    let name = f.name;
                    quote!(#name)
                }
            });
            let reconstructing = self.included_fields().map(|f| f.name);

            let FieldInfo {
                name: ref field_name,
                ty: ref field_type,
                ..
            } = field;
            let mut ty_generics: Vec<syn::GenericArgument> = self
                .generics
                .params
                .iter()
                .map(|generic_param| match generic_param {
                    syn::GenericParam::Type(type_param) => {
                        let ident = type_param.ident.clone();
                        syn::parse(quote!(#ident).into()).unwrap()
                    }
                    syn::GenericParam::Lifetime(lifetime_def) => {
                        syn::GenericArgument::Lifetime(lifetime_def.lifetime.clone())
                    }
                    syn::GenericParam::Const(const_param) => {
                        let ident = const_param.ident.clone();
                        syn::parse(quote!(#ident).into()).unwrap()
                    }
                })
                .collect();
            let mut target_generics_tuple = empty_type_tuple();
            let mut ty_generics_tuple = empty_type_tuple();
            let generics = self.modify_generics(|g| {
                let index_after_lifetime_in_generics = g
                    .params
                    .iter()
                    .filter(|arg| matches!(arg, syn::GenericParam::Lifetime(_)))
                    .count();
                for f in self.included_fields() {
                    if f.ordinal == field.ordinal {
                        ty_generics_tuple.elems.push_value(empty_type());
                        target_generics_tuple
                            .elems
                            .push_value(f.tuplized_type_ty_param());
                    } else {
                        g.params
                            .insert(index_after_lifetime_in_generics, f.generic_ty_param());
                        let generic_argument: syn::Type = f.type_ident();
                        ty_generics_tuple.elems.push_value(generic_argument.clone());
                        target_generics_tuple.elems.push_value(generic_argument);
                    }
                    ty_generics_tuple.elems.push_punct(Default::default());
                    target_generics_tuple.elems.push_punct(Default::default());
                }
            });
            let mut target_generics = ty_generics.clone();
            let index_after_lifetime_in_generics = target_generics
                .iter()
                .filter(|arg| matches!(arg, syn::GenericArgument::Lifetime(_)))
                .count();
            target_generics.insert(
                index_after_lifetime_in_generics,
                syn::GenericArgument::Type(target_generics_tuple.into()),
            );
            ty_generics.insert(
                index_after_lifetime_in_generics,
                syn::GenericArgument::Type(ty_generics_tuple.into()),
            );
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let doc = match field.builder_attr.setter.doc {
                Some(ref doc) => quote!(#[doc = #doc]),
                None => quote!(),
            };

            // NOTE: both auto_into and strip_option affect `arg_type` and `arg_expr`, but the order
            // of nesting is different so we have to do this little dance.
            let arg_type = if field.builder_attr.setter.strip_option.is_some()
                && field.builder_attr.setter.transform.is_none()
            {
                field.type_from_inside_option().ok_or_else(|| {
                    Error::new_spanned(
                        field_type,
                        "can't `strip_option` - field is not `Option<...>`",
                    )
                })?
            } else {
                field_type
            };
            let (arg_type, arg_expr) = if field.builder_attr.setter.auto_into.is_some() {
                (
                    quote!(impl ::core::convert::Into<#arg_type>),
                    quote!(#field_name.into()),
                )
            } else {
                (quote!(#arg_type), quote!(#field_name))
            };

            let (param_list, arg_expr) =
                if let Some(transform) = &field.builder_attr.setter.transform {
                    let params = transform.params.iter().map(|(pat, ty)| quote!(#pat: #ty));
                    let body = &transform.body;
                    (quote!(#(#params),*), quote!({ #body }))
                } else if field.builder_attr.setter.strip_option.is_some() {
                    (quote!(#field_name: #arg_type), quote!(Some(#arg_expr)))
                } else {
                    (quote!(#field_name: #arg_type), arg_expr)
                };

            let repeated_fields_error_type_name = syn::Ident::new(
                &format!(
                    "{}_Error_Repeated_field_{}",
                    builder_name,
                    strip_raw_ident_prefix(field_name.to_string())
                ),
                proc_macro2::Span::call_site(),
            );
            let repeated_fields_error_message = format!("Repeated field {field_name}");

            Ok(quote! {
                #[allow(dead_code, non_camel_case_types, missing_docs)]
                impl #impl_generics #builder_name < #( #ty_generics ),* > #where_clause {
                    #doc
                    pub fn #field_name (self, #param_list) -> #builder_name < #( #target_generics ),* > {
                        let #field_name = (#arg_expr,);
                        let ( #(#destructuring,)* ) = self.fields;
                        #builder_name {
                            fields: ( #(#reconstructing,)* ),
                            phantom: self.phantom,
                            attributes: self.attributes,
                        }
                    }
                }
                #[doc(hidden)]
                #[allow(dead_code, non_camel_case_types, non_snake_case)]
                pub enum #repeated_fields_error_type_name {}
                #[doc(hidden)]
                #[allow(dead_code, non_camel_case_types, missing_docs)]
                impl #impl_generics #builder_name < #( #target_generics ),* > #where_clause {
                    #[deprecated(
                        note = #repeated_fields_error_message
                    )]
                    pub fn #field_name (self, _: #repeated_fields_error_type_name) -> #builder_name < #( #target_generics ),* > {
                        self
                    }
                }
            })
        }

        pub fn required_field_impl(&self, field: &FieldInfo) -> Result<TokenStream, Error> {
            let StructInfo {
                ref name,
                ref builder_name,
                ..
            } = self;

            let FieldInfo {
                name: ref field_name,
                ..
            } = field;
            let mut builder_generics: Vec<syn::GenericArgument> = self
                .generics
                .params
                .iter()
                .map(|generic_param| match generic_param {
                    syn::GenericParam::Type(type_param) => {
                        let ident = &type_param.ident;
                        syn::parse(quote!(#ident).into()).unwrap()
                    }
                    syn::GenericParam::Lifetime(lifetime_def) => {
                        syn::GenericArgument::Lifetime(lifetime_def.lifetime.clone())
                    }
                    syn::GenericParam::Const(const_param) => {
                        let ident = &const_param.ident;
                        syn::parse(quote!(#ident).into()).unwrap()
                    }
                })
                .collect();
            let mut builder_generics_tuple = empty_type_tuple();
            let generics = self.modify_generics(|g| {
                let index_after_lifetime_in_generics = g
                    .params
                    .iter()
                    .filter(|arg| matches!(arg, syn::GenericParam::Lifetime(_)))
                    .count();
                for f in self.included_fields() {
                    if f.builder_attr.default.is_some() {
                        // `f` is not mandatory - it does not have it's own fake `build` method, so
                        // `field` will need to warn about missing `field`
                        // whether or not `f` is set.
                        assert!(
                            f.ordinal != field.ordinal,
                            "`required_field_impl` called for optional field {}",
                            field.name
                        );
                        g.params
                            .insert(index_after_lifetime_in_generics, f.generic_ty_param());
                        builder_generics_tuple.elems.push_value(f.type_ident());
                    } else if f.ordinal < field.ordinal {
                        // Only add a `build` method that warns about missing `field` if `f` is set.
                        // If `f` is not set, `f`'s `build` method will
                        // warn, since it appears earlier in the argument list.
                        builder_generics_tuple
                            .elems
                            .push_value(f.tuplized_type_ty_param());
                    } else if f.ordinal == field.ordinal {
                        builder_generics_tuple.elems.push_value(empty_type());
                    } else {
                        // `f` appears later in the argument list after `field`, so if they are both
                        // missing we will show a warning for `field` and
                        // not for `f` - which means this warning should appear whether
                        // or not `f` is set.
                        g.params
                            .insert(index_after_lifetime_in_generics, f.generic_ty_param());
                        builder_generics_tuple.elems.push_value(f.type_ident());
                    }

                    builder_generics_tuple.elems.push_punct(Default::default());
                }
            });

            let index_after_lifetime_in_generics = builder_generics
                .iter()
                .filter(|arg| matches!(arg, syn::GenericArgument::Lifetime(_)))
                .count();
            builder_generics.insert(
                index_after_lifetime_in_generics,
                syn::GenericArgument::Type(builder_generics_tuple.into()),
            );
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let (_, ty_generics, _) = self.generics.split_for_impl();

            let early_build_error_type_name = syn::Ident::new(
                &format!(
                    "{}_Error_Missing_required_field_{}",
                    builder_name,
                    strip_raw_ident_prefix(field_name.to_string())
                ),
                proc_macro2::Span::call_site(),
            );
            let early_build_error_message = format!("Missing required field {field_name}");

            Ok(quote! {
                #[doc(hidden)]
                #[allow(dead_code, non_camel_case_types, non_snake_case)]
                pub enum #early_build_error_type_name {}
                #[doc(hidden)]
                #[allow(dead_code, non_camel_case_types, missing_docs, clippy::panic)]
                impl #impl_generics #builder_name < #( #builder_generics ),* > #where_clause {
                    #[deprecated(
                        note = #early_build_error_message
                    )]
                    pub fn build(self, _: #early_build_error_type_name) -> #name #ty_generics {
                        panic!();
                    }
                }
            })
        }

        pub fn build_method_impl(&self) -> TokenStream {
            let StructInfo {
                ref name,
                ref builder_name,
                ref attributes,
                ..
            } = self;

            let generics = self.modify_generics(|g| {
                let index_after_lifetime_in_generics = g
                    .params
                    .iter()
                    .filter(|arg| matches!(arg, syn::GenericParam::Lifetime(_)))
                    .count();
                for field in self.included_fields() {
                    if field.builder_attr.default.is_some() {
                        let trait_ref = syn::TraitBound {
                            paren_token: None,
                            lifetimes: None,
                            modifier: syn::TraitBoundModifier::None,
                            path: syn::PathSegment {
                                ident: self.conversion_helper_trait_name.clone(),
                                arguments: syn::PathArguments::AngleBracketed(
                                    syn::AngleBracketedGenericArguments {
                                        colon2_token: None,
                                        lt_token: Default::default(),
                                        args: make_punctuated_single(syn::GenericArgument::Type(
                                            field.ty.clone(),
                                        )),
                                        gt_token: Default::default(),
                                    },
                                ),
                            }
                            .into(),
                        };
                        let mut generic_param: syn::TypeParam = field.generic_ident.clone().into();
                        generic_param.bounds.push(trait_ref.into());
                        g.params
                            .insert(index_after_lifetime_in_generics, generic_param.into());
                    }
                }
            });
            let (impl_generics, _, _) = generics.split_for_impl();

            let (_, ty_generics, where_clause) = self.generics.split_for_impl();

            let modified_ty_generics = modify_types_generics_hack(&ty_generics, |args| {
                args.insert(
                    0,
                    syn::GenericArgument::Type(
                        type_tuple(self.included_fields().map(|field| {
                            if field.builder_attr.default.is_some() {
                                field.type_ident()
                            } else {
                                field.tuplized_type_ty_param()
                            }
                        }))
                        .into(),
                    ),
                );
            });

            let destructuring = self.included_fields().map(|f| f.name);

            let helper_trait_name = &self.conversion_helper_trait_name;
            // The default of a field can refer to earlier-defined fields, which we handle by
            // writing out a bunch of `let` statements first, which can each refer to earlier ones.
            // This means that field ordering may actually be significant, which isn't ideal. We
            // could relax that restriction by calculating a DAG of field default
            // dependencies and reordering based on that, but for now this much simpler
            // thing is a reasonable approach.
            let assignments = self.fields.iter().map(|field| {
                let name = &field.name;
                if let Some(ref default) = field.builder_attr.default {
                    if field.builder_attr.setter.skip.is_some() {
                        quote!(let #name = #default;)
                    } else {
                        quote!(let #name = #helper_trait_name::into_value(#name, || #default);)
                    }
                } else {
                    quote!(let #name = #name.0;)
                }
            });
            let field_names = self.fields.iter().map(|field| field.name);
            let doc = if self.builder_attr.doc {
                match self.builder_attr.build_method_doc {
                    Some(ref doc) => quote!(#[doc = #doc]),
                    None => {
                        // I'd prefer “a” or “an” to “its”, but determining which is grammatically
                        // correct is roughly impossible.
                        let doc =
                            format!("Finalize the builder and create its [`{name}`] instance");
                        quote!(#[doc = #doc])
                    }
                }
            } else {
                quote!()
            };

            let attributes_field = if attributes.is_some() {
                quote! { attributes: self.attributes, }
            } else {
                quote! {}
            };
            quote!(
                #[allow(dead_code, non_camel_case_types, missing_docs)]
                impl #impl_generics #builder_name #modified_ty_generics #where_clause {
                    #doc
                    #[allow(clippy::default_trait_access)]
                    pub fn build(self) -> #name #ty_generics {
                        let ( #(#destructuring,)* ) = self.fields;
                        #( #assignments )*
                        #name {
                            #( #field_names, )*
                            #attributes_field
                        }
                    }
                }
            )
        }
    }

    #[derive(Debug, Default)]
    pub struct TypeBuilderAttr {
        /// Whether to show docs for the `TypeBuilder` type (rather than hiding them).
        pub doc: bool,

        /// Docs on the `Type::builder()` method.
        pub builder_method_doc: Option<syn::Expr>,

        /// Docs on the `TypeBuilder` type. Specifying this implies `doc`, but you can just specify
        /// `doc` instead and a default value will be filled in here.
        pub builder_type_doc: Option<syn::Expr>,

        /// Docs on the `TypeBuilder.build()` method. Specifying this implies `doc`, but you can
        /// just specify `doc` instead and a default value will be filled in here.
        pub build_method_doc: Option<syn::Expr>,

        pub field_defaults: FieldBuilderAttr,
    }

    impl TypeBuilderAttr {
        pub fn new(attrs: &[syn::Attribute]) -> Result<TypeBuilderAttr, Error> {
            let mut result = TypeBuilderAttr::default();
            for attr in attrs {
                if !attr.path().is_ident("prop") {
                    continue;
                }
                let as_expr: Punctuated<syn::Expr, Token![,]> =
                    attr.parse_args_with(Punctuated::parse_terminated)?;
                for expr in as_expr {
                    result.apply_meta(expr)?;
                }
            }

            Ok(result)
        }

        fn apply_meta(&mut self, expr: syn::Expr) -> Result<(), Error> {
            match expr {
                syn::Expr::Assign(assign) => {
                    let name = expr_to_single_string(&assign.left)
                        .ok_or_else(|| Error::new_spanned(&assign.left, "Expected identifier"))?;
                    match name.as_str() {
                        "builder_method_doc" => {
                            self.builder_method_doc = Some(*assign.right);
                            Ok(())
                        }
                        "builder_type_doc" => {
                            self.builder_type_doc = Some(*assign.right);
                            self.doc = true;
                            Ok(())
                        }
                        "build_method_doc" => {
                            self.build_method_doc = Some(*assign.right);
                            self.doc = true;
                            Ok(())
                        }
                        _ => Err(Error::new_spanned(
                            &assign,
                            format!("Unknown parameter {name:?}"),
                        )),
                    }
                }
                syn::Expr::Path(path) => {
                    let name = path_to_single_string(&path.path)
                        .ok_or_else(|| Error::new_spanned(&path, "Expected identifier"))?;
                    match name.as_str() {
                        "doc" => {
                            self.doc = true;
                            Ok(())
                        }
                        _ => Err(Error::new_spanned(
                            &path,
                            format!("Unknown parameter {name:?}"),
                        )),
                    }
                }
                syn::Expr::Call(call) => {
                    let subsetting_name = if let syn::Expr::Path(path) = &*call.func {
                        path_to_single_string(&path.path)
                    } else {
                        None
                    }
                    .ok_or_else(|| {
                        let call_func = &call.func;
                        let call_func = quote!(#call_func);
                        Error::new_spanned(
                            &call.func,
                            format!("Illegal prop setting group {call_func}"),
                        )
                    })?;
                    match subsetting_name.as_str() {
                        "field_defaults" => {
                            for arg in call.args {
                                self.field_defaults.apply_meta(arg)?;
                            }
                            Ok(())
                        }
                        _ => Err(Error::new_spanned(
                            &call.func,
                            format!("Illegal prop setting group name {subsetting_name}"),
                        )),
                    }
                }
                _ => Err(Error::new_spanned(expr, "Expected (<...>=<...>)")),
            }
        }
    }
}

mod field_info {
    use proc_macro2::{Span, TokenStream};
    use quote::quote;
    use syn::parse::Error;
    use syn::punctuated::Punctuated;
    use syn::spanned::Spanned;
    use syn::Token;

    use super::util::{
        expr_to_single_string, ident_to_type, path_to_single_string, strip_raw_ident_prefix,
        type_from_inside_option,
    };

    #[derive(Debug)]
    pub struct FieldInfo<'a> {
        pub ordinal: usize,
        pub name: &'a syn::Ident,
        pub generic_ident: syn::Ident,
        pub ty: &'a syn::Type,
        pub builder_attr: FieldBuilderAttr,
    }

    impl FieldInfo<'_> {
        pub fn new(
            ordinal: usize,
            field: &syn::Field,
            field_defaults: FieldBuilderAttr,
        ) -> Result<FieldInfo<'_>, Error> {
            if let Some(ref name) = field.ident {
                let mut builder_attr = field_defaults.with(&field.attrs)?;

                let strip_option_auto = builder_attr.setter.strip_option.is_some()
                    || !builder_attr.ignore_option && type_from_inside_option(&field.ty).is_some();
                if builder_attr.setter.strip_option.is_none() && strip_option_auto {
                    builder_attr.default =
                        Some(syn::parse_quote!(::std::default::Default::default()));
                    builder_attr.setter.strip_option = Some(field.ty.span());
                } else if name == "children" || name == "attributes" {
                    // If this field is the `children` or `attributes` field, make it implicitly
                    // have a default value.
                    builder_attr.default =
                        Some(syn::parse_quote! { ::std::default::Default::default() });
                }

                Ok(FieldInfo {
                    ordinal,
                    name,
                    generic_ident: syn::Ident::new(
                        &format!("__{}", strip_raw_ident_prefix(name.to_string())),
                        Span::call_site(),
                    ),
                    ty: &field.ty,
                    builder_attr,
                })
            } else {
                Err(Error::new(field.span(), "Nameless field in struct"))
            }
        }

        pub fn generic_ty_param(&self) -> syn::GenericParam {
            syn::GenericParam::Type(self.generic_ident.clone().into())
        }

        pub fn type_ident(&self) -> syn::Type {
            ident_to_type(self.generic_ident.clone())
        }

        pub fn tuplized_type_ty_param(&self) -> syn::Type {
            let mut types = syn::punctuated::Punctuated::default();
            types.push(self.ty.clone());
            types.push_punct(Default::default());
            syn::TypeTuple {
                paren_token: Default::default(),
                elems: types,
            }
            .into()
        }

        pub fn type_from_inside_option(&self) -> Option<&syn::Type> {
            type_from_inside_option(self.ty)
        }
    }

    #[derive(Debug, Default, Clone)]
    pub struct FieldBuilderAttr {
        pub default: Option<syn::Expr>,
        pub ignore_option: bool,
        pub setter: SetterSettings,
        /// Example: `#[prop(attributes(html, div))]`
        pub attributes: Option<(AttributeBase, String)>,
    }

    #[derive(Debug, Default, Clone)]
    pub struct SetterSettings {
        pub doc: Option<syn::Expr>,
        pub skip: Option<Span>,
        pub auto_into: Option<Span>,
        pub strip_option: Option<Span>,
        pub transform: Option<Transform>,
    }

    #[derive(Debug, Clone)]
    pub enum AttributeBase {
        Html,
        Svg,
    }

    impl FieldBuilderAttr {
        pub fn with(mut self, attrs: &[syn::Attribute]) -> Result<Self, Error> {
            for attr in attrs {
                if !attr.path().is_ident("prop") {
                    continue;
                }
                let as_expr: Punctuated<syn::Expr, Token![,]> =
                    attr.parse_args_with(Punctuated::parse_terminated)?;
                for expr in as_expr {
                    self.apply_meta(expr)?;
                }
            }

            self.inter_fields_conflicts()?;

            Ok(self)
        }

        pub fn apply_meta(&mut self, expr: syn::Expr) -> Result<(), Error> {
            match expr {
                syn::Expr::Assign(assign) => {
                    let name = expr_to_single_string(&assign.left)
                        .ok_or_else(|| Error::new_spanned(&assign.left, "Expected identifier"))?;
                    match name.as_str() {
                        "default" => {
                            self.default = Some(*assign.right);
                            Ok(())
                        }
                        "default_code" => {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(code),
                                ..
                            }) = *assign.right
                            {
                                use std::str::FromStr;
                                let tokenized_code = TokenStream::from_str(&code.value())?;
                                self.default = Some(
                                    syn::parse(tokenized_code.into())
                                        .map_err(|e| Error::new_spanned(code, format!("{e}")))?,
                                );
                            } else {
                                return Err(Error::new_spanned(assign.right, "Expected string"));
                            }
                            Ok(())
                        }
                        _ => Err(Error::new_spanned(
                            &assign,
                            format!("Unknown parameter {name:?}"),
                        )),
                    }
                }
                syn::Expr::Path(path) => {
                    let name = path_to_single_string(&path.path)
                        .ok_or_else(|| Error::new_spanned(&path, "Expected identifier"))?;
                    match name.as_str() {
                        "default" => {
                            self.default = Some(
                                syn::parse(quote!(::core::default::Default::default()).into())
                                    .unwrap(),
                            );
                            Ok(())
                        }
                        _ => Err(Error::new_spanned(
                            &path,
                            format!("Unknown parameter {name:?}"),
                        )),
                    }
                }
                syn::Expr::Call(call) => {
                    let subsetting_name = if let syn::Expr::Path(path) = &*call.func {
                        path_to_single_string(&path.path)
                    } else {
                        None
                    }
                    .ok_or_else(|| {
                        let call_func = &call.func;
                        let call_func = quote!(#call_func);
                        Error::new_spanned(
                            &call.func,
                            format!("Illegal prop setting group {call_func}"),
                        )
                    })?;
                    match subsetting_name.as_ref() {
                        "setter" => {
                            for arg in call.args {
                                self.setter.apply_meta(arg)?;
                            }
                            Ok(())
                        }
                        "attributes" => {
                            let args = call.args.iter().collect::<Vec<_>>();
                            if args.len() != 2 {
                                Err(Error::new_spanned(
                                    &call.args,
                                    "Expected 2 arguments for `attributes`",
                                ))
                            } else {
                                let arg0 = expr_to_single_string(args[0]);
                                let arg1 = expr_to_single_string(args[1]);
                                if let (Some(arg0), Some(arg1)) = (arg0, arg1) {
                                    self.attributes = match arg0.as_str() {
                                        "html" => Some((AttributeBase::Html, arg1)),
                                        "svg" => Some((AttributeBase::Svg, arg1)),
                                        _ => {
                                            return Err(Error::new_spanned(
                                                args[0],
                                                "The first argument to `attributes` should be either `html` or `svg",
                                            ));
                                        }
                                    };
                                    Ok(())
                                } else {
                                    Err(Error::new_spanned(
                                        &call.args,
                                        "Arguments to `attributes` should be identifiers",
                                    ))
                                }
                            }
                        }
                        _ => Err(Error::new_spanned(
                            &call.func,
                            format!("Illegal prop setting group name {subsetting_name}"),
                        )),
                    }
                }
                syn::Expr::Unary(syn::ExprUnary {
                    op: syn::UnOp::Not(_),
                    expr,
                    ..
                }) => {
                    if let syn::Expr::Path(path) = *expr {
                        let name = path_to_single_string(&path.path)
                            .ok_or_else(|| Error::new_spanned(&path, "Expected identifier"))?;
                        match name.as_str() {
                            "default" => {
                                self.default = None;
                                Ok(())
                            }
                            "optional" => {
                                self.ignore_option = true;
                                Ok(())
                            }
                            _ => Err(Error::new_spanned(path, "Unknown setting".to_owned())),
                        }
                    } else {
                        Err(Error::new_spanned(
                            expr,
                            "Expected simple identifier".to_owned(),
                        ))
                    }
                }
                _ => Err(Error::new_spanned(expr, "Expected (<...>=<...>)")),
            }
        }

        fn inter_fields_conflicts(&self) -> Result<(), Error> {
            if let (Some(skip), None) = (&self.setter.skip, &self.default) {
                return Err(Error::new(
                    *skip,
                    "#[prop(skip)] must be accompanied by default or default_code",
                ));
            }

            if let (Some(strip_option), Some(transform)) =
                (&self.setter.strip_option, &self.setter.transform)
            {
                let mut error = Error::new(transform.span, "transform conflicts with strip_option");
                error.combine(Error::new(*strip_option, "strip_option set here"));
                return Err(error);
            }
            Ok(())
        }
    }

    impl SetterSettings {
        fn apply_meta(&mut self, expr: syn::Expr) -> Result<(), Error> {
            match expr {
                syn::Expr::Assign(assign) => {
                    let name = expr_to_single_string(&assign.left)
                        .ok_or_else(|| Error::new_spanned(&assign.left, "Expected identifier"))?;
                    match name.as_str() {
                        "doc" => {
                            self.doc = Some(*assign.right);
                            Ok(())
                        }
                        "transform" => {
                            // if self.strip_option.is_some() {
                            // return Err(Error::new(assign.span(), "Illegal setting - transform
                            // conflicts with strip_option")); }
                            self.transform =
                                Some(parse_transform_closure(assign.left.span(), &assign.right)?);
                            Ok(())
                        }
                        _ => Err(Error::new_spanned(
                            &assign,
                            format!("Unknown parameter {name:?}"),
                        )),
                    }
                }
                syn::Expr::Path(path) => {
                    let name = path_to_single_string(&path.path)
                        .ok_or_else(|| Error::new_spanned(&path, "Expected identifier"))?;
                    macro_rules! handle_fields {
                    ( $( $flag:expr, $field:ident, $already:expr, $checks:expr; )* ) => {
                        match name.as_str() {
                            $(
                                $flag => {
                                    if self.$field.is_some() {
                                        Err(Error::new(path.span(), concat!("Illegal setting - field is already ", $already)))
                                    } else {
                                        $checks;
                                        self.$field = Some(path.span());
                                        Ok(())
                                    }
                                }
                            )*
                            _ => Err(Error::new_spanned(
                                    &path,
                                    format!("Unknown setter parameter {:?}", name),
                            ))
                        }
                    }
                }
                    handle_fields!(
                        "skip", skip, "skipped", {};
                        "into", auto_into, "calling into() on the argument", {};
                        "strip_option", strip_option, "putting the argument in Some(...)", {
                            // if self.transform.is_some() {
                                // let mut error = Error::new(path.span(), "Illegal setting - strip_option conflicts with transform");
                                // error.combine(Error::new(self.transform.as_ref().unwrap().body.span(), "yup"));
                                // return Err(error);
                            // }
                        };
                    )
                }
                syn::Expr::Unary(syn::ExprUnary {
                    op: syn::UnOp::Not(_),
                    expr,
                    ..
                }) => {
                    if let syn::Expr::Path(path) = *expr {
                        let name = path_to_single_string(&path.path)
                            .ok_or_else(|| Error::new_spanned(&path, "Expected identifier"))?;
                        match name.as_str() {
                            "doc" => {
                                self.doc = None;
                                Ok(())
                            }
                            "skip" => {
                                self.skip = None;
                                Ok(())
                            }
                            "auto_into" => {
                                self.auto_into = None;
                                Ok(())
                            }
                            "strip_option" => {
                                self.strip_option = None;
                                Ok(())
                            }
                            _ => Err(Error::new_spanned(path, "Unknown setting".to_owned())),
                        }
                    } else {
                        Err(Error::new_spanned(
                            expr,
                            "Expected simple identifier".to_owned(),
                        ))
                    }
                }
                _ => Err(Error::new_spanned(expr, "Expected (<...>=<...>)")),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Transform {
        pub params: Vec<(syn::Pat, syn::Type)>,
        pub body: syn::Expr,
        span: Span,
    }

    fn parse_transform_closure(span: Span, expr: &syn::Expr) -> Result<Transform, Error> {
        let closure = match expr {
            syn::Expr::Closure(closure) => closure,
            _ => return Err(Error::new_spanned(expr, "Expected closure")),
        };
        if let Some(kw) = &closure.asyncness {
            return Err(Error::new(kw.span, "Transform closure cannot be async"));
        }
        if let Some(kw) = &closure.capture {
            return Err(Error::new(kw.span, "Transform closure cannot be move"));
        }

        let params = closure
            .inputs
            .iter()
            .map(|input| match input {
                syn::Pat::Type(pat_type) => Ok((
                    syn::Pat::clone(&pat_type.pat),
                    syn::Type::clone(&pat_type.ty),
                )),
                _ => Err(Error::new_spanned(
                    input,
                    "Transform closure must explicitly declare types",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let body = &closure.body;

        Ok(Transform {
            params,
            body: syn::Expr::clone(body),
            span,
        })
    }
}

mod util {
    use quote::ToTokens;

    pub fn path_to_single_string(path: &syn::Path) -> Option<String> {
        if path.leading_colon.is_some() {
            return None;
        }
        let mut it = path.segments.iter();
        let segment = it.next()?;
        if it.next().is_some() {
            // Multipart path
            return None;
        }
        if segment.arguments != syn::PathArguments::None {
            return None;
        }
        Some(segment.ident.to_string())
    }

    pub fn expr_to_single_string(expr: &syn::Expr) -> Option<String> {
        if let syn::Expr::Path(path) = expr {
            path_to_single_string(&path.path)
        } else {
            None
        }
    }

    pub fn ident_to_type(ident: syn::Ident) -> syn::Type {
        let mut path = syn::Path {
            leading_colon: None,
            segments: Default::default(),
        };
        path.segments.push(syn::PathSegment {
            ident,
            arguments: Default::default(),
        });
        syn::Type::Path(syn::TypePath { qself: None, path })
    }

    pub fn empty_type() -> syn::Type {
        syn::TypeTuple {
            paren_token: Default::default(),
            elems: Default::default(),
        }
        .into()
    }

    pub fn type_tuple(elems: impl Iterator<Item = syn::Type>) -> syn::TypeTuple {
        let mut result = syn::TypeTuple {
            paren_token: Default::default(),
            elems: elems.collect(),
        };
        if !result.elems.empty_or_trailing() {
            result.elems.push_punct(Default::default());
        }
        result
    }

    pub fn empty_type_tuple() -> syn::TypeTuple {
        syn::TypeTuple {
            paren_token: Default::default(),
            elems: Default::default(),
        }
    }

    pub fn make_punctuated_single<T, P: Default>(value: T) -> syn::punctuated::Punctuated<T, P> {
        let mut punctuated = syn::punctuated::Punctuated::new();
        punctuated.push(value);
        punctuated
    }

    pub fn modify_types_generics_hack<F>(
        ty_generics: &syn::TypeGenerics,
        mut mutator: F,
    ) -> syn::AngleBracketedGenericArguments
    where
        F: FnMut(&mut syn::punctuated::Punctuated<syn::GenericArgument, syn::token::Comma>),
    {
        let mut abga: syn::AngleBracketedGenericArguments =
            syn::parse(ty_generics.clone().into_token_stream().into()).unwrap_or_else(|_| {
                syn::AngleBracketedGenericArguments {
                    colon2_token: None,
                    lt_token: Default::default(),
                    args: Default::default(),
                    gt_token: Default::default(),
                }
            });
        mutator(&mut abga.args);
        abga
    }

    pub fn strip_raw_ident_prefix(mut name: String) -> String {
        if name.starts_with("r#") {
            name.replace_range(0..2, "");
        }
        name
    }

    pub fn type_from_inside_option(ty: &syn::Type) -> Option<&syn::Type> {
        let path = if let syn::Type::Path(type_path) = ty {
            if type_path.qself.is_some() {
                return None;
            } else {
                &type_path.path
            }
        } else {
            return None;
        };
        let segment = path.segments.last()?;
        if segment.ident != "Option" {
            return None;
        }
        let generic_params =
            if let syn::PathArguments::AngleBracketed(generic_params) = &segment.arguments {
                generic_params
            } else {
                return None;
            };
        if let syn::GenericArgument::Type(ty) = generic_params.args.first()? {
            Some(ty)
        } else {
            None
        }
    }
}
