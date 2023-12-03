//! Codegen for `view!` macro.
//!
//! Implementation note: We are not using the `quote::ToTokens` trait because we need to pass
//! additional information to the codegen such as which mode (Client, Hydrate, SSR), etc...

use std::collections::HashSet;

use once_cell::sync::Lazy;
use proc_macro2::TokenStream;
use quote::quote;
use sycamore_view_parser::ir::{DynNode, Node, Prop, PropType, Root, TagIdent, TagNode, TextNode};
use syn::{Expr, ExprLit, Ident, Lit, LitBool};

pub struct Codegen {
    // TODO: configure mode: Client, Hydrate, SSR
}

impl Codegen {
    pub fn root(&self, root: &Root) -> TokenStream {
        match &root.0[..] {
            [] => quote! {
                ::sycamore::view::View::empty()
            },
            [node] => self.node(node),
            nodes => {
                let nodes = nodes.iter().map(|node| self.node(node));
                quote! {
                    ::sycamore::view::View::new_fragment({
                        ::std::vec![
                            #(#nodes),*
                        ]
                    })
                }
            }
        }
    }

    /// Generate a `View` from a `Node`. If the root is an element, create a `Template`.
    pub fn node(&self, node: &Node) -> TokenStream {
        match node {
            Node::Tag(tag) => {
                if is_component(&tag.ident) {
                    impl_component(tag)
                } else {
                    let template_id = rand::random::<u32>();

                    let mut codegen = CodegenTemplate::new();

                    let shape = codegen.node(node);
                    let dyn_values = codegen.dyn_values;
                    let flagged_nodes_quoted = codegen.flagged_nodes_quoted;
                    quote! {{
                        use ::sycamore::generic_node::SycamoreElement as _;

                        static __TEMPLATE: ::sycamore::generic_node::Template = ::sycamore::generic_node::Template {
                            id: ::sycamore::generic_node::TemplateId(#template_id),
                            shape: #shape,
                        };

                        let __dyn_values = ::std::vec![#(#dyn_values),*];
                        let __result = ::sycamore::generic_node::__instantiate_template(&__TEMPLATE);
                        ::sycamore::generic_node::__apply_dyn_values_to_template(&__result.dyn_markers, __dyn_values);
                        let __flagged = __result.flagged_nodes;
                        #flagged_nodes_quoted

                        ::sycamore::view::View::new_node(__result.root)
                    }}
                }
            }
            Node::Text(TextNode { value }) => quote! {
                ::sycamore::view::View::new_node(::sycamore::generic_node::GenericNode::text_node(::std::borrow::Cow::Borrowed(#value)))
            },
            Node::Dyn(DynNode { value }) => {
                quote! {
                    ::sycamore::view::View::new_dyn(move || ::sycamore::view::ToView::to_view(&(#value)))
                }
            }
        }
    }
}

/// Codegen a `TemplateShape`.
struct CodegenTemplate {
    flag_counter: usize,
    flagged_nodes_quoted: TokenStream,
    dyn_values: Vec<TokenStream>,
}

impl CodegenTemplate {
    fn new() -> Self {
        Self {
            flag_counter: 0,
            flagged_nodes_quoted: TokenStream::new(),
            dyn_values: Vec::new(),
        }
    }

    fn node(&mut self, node: &Node) -> TokenStream {
        match node {
            Node::Tag(tag) => {
                if is_component(&tag.ident) {
                    self.component(tag)
                } else {
                    self.element(tag)
                }
            }
            Node::Text(text) => self.text(text),
            Node::Dyn(dyn_node) => self.dyn_marker(dyn_node),
        }
    }

    fn element(&mut self, node: &TagNode) -> TokenStream {
        let TagNode {
            ident,
            props,
            children,
        } = node;

        let attrs = props
            .iter()
            .map(|attr| self.attribute(attr))
            .collect::<Vec<_>>();
        let flag = attrs.iter().any(|(_, flag)| *flag);
        let attrs = attrs.into_iter().filter_map(|(attr, _)| attr);

        if flag {
            self.flag_counter += 1;
        }

        // Make sure we don't have both children and dangerously_set_inner_html.
        let has_children = !children.0.is_empty();
        let has_dangerously_set_inner_html = props.iter().any(|prop| {
            matches!(
                &prop.ty,
                PropType::Plain { ident } if ident == "dangerously_set_inner_html"
            )
        });
        if has_children && has_dangerously_set_inner_html {
            return syn::Error::new(
                ident.span(),
                "children and `dangerously_set_inner_html` cannot both be set",
            )
            .to_compile_error();
        }

        // We run codegen for children after attrs to make sure that we have the correct flag
        // counter.
        let children = children
            .0
            .iter()
            .map(|child| self.node(child))
            .collect::<Vec<_>>();

        match ident {
            TagIdent::Path(tag) => {
                assert!(tag.get_ident().is_some(), "elements must be an ident");
                quote! {{
                    type __tag = ::sycamore::web::html::#tag;
                    ::sycamore::generic_node::TemplateShape::Element {
                        tag: __tag::TAG_NAME,
                        ns: __tag::NAMESPACE,
                        children: &[#(#children),*],
                        attributes: &[#(#attrs),*],
                        flag: #flag,
                    }
                }}
            }
            TagIdent::Hyphenated(tag) => quote! {
                ::sycamore::generic_node::TemplateShape::Element {
                    tag: #tag,
                    ns: ::std::option::Option::None,
                    children: &[#(#children),*],
                    attributes: &[#(#attrs),*],
                    flag: #flag,
                }
            },
        }
    }

    fn attribute(&mut self, attr: &Prop) -> (Option<TokenStream>, bool) {
        let flag_counter = self.flag_counter;

        let value = &attr.value;

        let is_dynamic = !matches!(value, Expr::Lit(ExprLit { .. }));

        match &attr.ty {
            PropType::Plain { .. } | PropType::PlainHyphenated { .. } => {
                let ident = match &attr.ty {
                    PropType::Plain { ident } => ident.to_string(),
                    PropType::PlainHyphenated { ident } => ident.clone(),
                    _ => unreachable!(),
                };
                if ident == "dangerously_set_inner_html" {
                    if is_dynamic {
                        self.flagged_nodes_quoted.extend(quote! {
                            let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                            ::sycamore::reactive::create_effect(move || {
                                ::sycamore::generic_node::GenericNode::dangerously_set_inner_html(
                                    &__el,
                                    <_ as ::std::convert::Into<_>>::into(#value),
                                );
                            });
                        });
                        (None, true)
                    } else {
                        self.flagged_nodes_quoted.extend(quote! {
                            ::sycamore::generic_node::GenericNode::dangerously_set_inner_html(
                                &__flagged[#flag_counter],
                                <_ as ::std::convert::Into<_>>::into(#value),
                            );
                        });
                        (None, true)
                    }
                } else if is_bool_attr(&ident.to_string()) {
                    if is_dynamic {
                        let quoted_set_attribute = quote! {
                            if #value {
                                ::sycamore::generic_node::GenericNode::set_attribute(&__el, ::std::borrow::Cow::Borrowed(#ident), ::std::borrow::Cow::Borrowed(""));
                            } else {
                                ::sycamore::generic_node::GenericNode::remove_attribute(&__el, ::std::borrow::Cow::Borrowed(#ident));
                            }
                        };
                        self.flagged_nodes_quoted.extend(quote! {
                            let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                            ::sycamore::reactive::create_effect(move || { #quoted_set_attribute });
                        });
                        (None, true)
                    } else if let Expr::Lit(ExprLit {
                        lit: Lit::Bool(LitBool { value, .. }),
                        ..
                    }) = value
                    {
                        let stringified = match value {
                            true => "true",
                            false => "false",
                        };
                        (
                            Some(quote! { (#ident, ::std::borrow::Cow::Borrowed(#stringified)) }),
                            false,
                        )
                    } else {
                        // Wrong type. Produce a type error.
                        (
                            Some(
                                quote! { (#ident, { let _e: ::std::primitive::bool = #value; ::std::borrow::Cow::Borrowed("") }) },
                            ),
                            false,
                        )
                    }
                } else {
                    // Normal string attribute.
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(text),
                        ..
                    }) = value
                    {
                        (
                            Some(quote! { (#ident, ::std::borrow::Cow::Borrowed(#text)) }),
                            false,
                        )
                    } else {
                        let text = quote! { ::std::borrow::Cow::Owned(::std::string::ToString::to_string(&#value)) };
                        if is_dynamic {
                            self.flagged_nodes_quoted.extend(quote! {
                                let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                                ::sycamore::reactive::create_effect(move || {
                                    ::sycamore::generic_node::GenericNode::set_attribute(&__el, ::std::borrow::Cow::Borrowed(#ident), #text);
                                });
                            });
                        } else {
                            self.flagged_nodes_quoted.extend(quote! {
                                ::sycamore::generic_node::GenericNode::set_attribute(&__flagged[#flag_counter], ::std::borrow::Cow::Borrowed(#ident), #text);
                            });
                        }
                        (None, true)
                    }
                }
            }
            PropType::Directive { dir, ident } => match dir.to_string().as_str() {
                "on" => {
                    self.flagged_nodes_quoted.extend(quote! {
                        ::sycamore::generic_node::GenericNode::event(
                            &__flagged[#flag_counter],
                            ::sycamore::web::html::ev::#ident,
                            #value,
                        );
                    });
                    (None, true)
                }
                "prop" => {
                    let ident = ident.to_string();
                    let value =
                        quote! { ::std::convert::Into::<::sycamore::rt::JsValue>::into(#value) };

                    if is_dynamic {
                        self.flagged_nodes_quoted.extend(quote! {
                        let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                        ::sycamore::reactive::create_effect(move ||
                            ::sycamore::generic_node::GenericNode::set_property(&__el, #ident, &#value)
                        );
                    });
                    } else {
                        self.flagged_nodes_quoted.extend(quote! {
                        ::sycamore::generic_node::GenericNode::set_property(&__flagged[#flag_counter], #ident, &#value);
                    });
                    }

                    (None, true)
                }
                "bind" => {
                    #[derive(Clone, Copy)]
                    enum JsPropertyType {
                        Bool,
                        String,
                        Number,
                    }

                    let span = ident.span();
                    let ident = ident.to_string();
                    let (event_name, property_ty) = match ident.as_str() {
                        "value" => (quote! { input }, JsPropertyType::String),
                        "valueAsNumber" => (quote! { input }, JsPropertyType::Number),
                        "checked" => (quote! { change }, JsPropertyType::Bool),
                        _ => {
                            self.flagged_nodes_quoted.extend(
                                syn::Error::new(
                                    span,
                                    format!("property `{}` is not supported with `bind:`", ident),
                                )
                                .to_compile_error(),
                            );
                            return (None, false);
                        }
                    };

                    let convert_into_jsvalue_fn = match property_ty {
                        JsPropertyType::Bool => quote! {
                            ::sycamore::rt::JsValue::from_bool(__expr.get())
                        },
                        JsPropertyType::Number => quote! {
                            ::sycamore::rt::JsValue::from_f64(__expr.get())
                        },
                        JsPropertyType::String => quote! {
                            __expr.with(|__expr|
                                ::sycamore::rt::JsValue::from_str(&::std::string::ToString::to_string(__expr))
                            )
                        },
                    };

                    let event_target_prop = quote! {
                        ::sycamore::rt::Reflect::get(
                            &event.target().unwrap(),
                            &::std::convert::Into::<::sycamore::rt::JsValue>::into(#ident)
                        ).unwrap()
                    };

                    let convert_from_jsvalue_fn = match property_ty {
                        JsPropertyType::Bool => quote! {
                            ::sycamore::rt::JsValue::as_bool(&#event_target_prop).unwrap()
                        },
                        JsPropertyType::Number => quote! {
                            ::sycamore::rt::JsValue::as_f64(&#event_target_prop).unwrap()
                        },
                        JsPropertyType::String => quote! {
                            ::sycamore::rt::JsValue::as_string(&#event_target_prop).unwrap()
                        },
                    };

                    self.flagged_nodes_quoted.extend(quote! {
                        #[cfg(target_arch = "wasm32")]
                        {
                            let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                            ::sycamore::reactive::create_effect({
                                let __expr = ::std::clone::Clone::clone(&#value);
                                move ||::sycamore::generic_node::GenericNode::set_property(
                                    &__el,
                                    #ident,
                                    &#convert_into_jsvalue_fn,
                                )
                            });
                        }
                        ::sycamore::generic_node::GenericNode::event(
                            &__flagged[#flag_counter], ::sycamore::web::html::ev::#event_name,
                            {
                                let __expr = ::std::clone::Clone::clone(&#value);
                                ::std::boxed::Box::new(move |event: ::sycamore::rt::Event| {
                                    ::sycamore::reactive::Signal::set(__expr, #convert_from_jsvalue_fn);
                                })
                            },
                        );
                    });
                    (None, true)
                }
                _ => (
                    Some(
                        syn::Error::new(dir.span(), format!("unknown directive `{dir}`"))
                            .to_compile_error(),
                    ),
                    false,
                ),
            },
            PropType::Ref => {
                self.flagged_nodes_quoted.extend(quote! {
                        ::sycamore::noderef::NodeRef::set(&#value, ::std::clone::Clone::clone(&__flagged[#flag_counter]));
                    });
                (None, true)
            }
            PropType::Spread => {
                self.flagged_nodes_quoted.extend(quote! {
                    let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                    for (name, value) in #value.drain() {
                        let __el = ::std::clone::Clone::clone(&__flagged[#flag_counter]);
                        ::sycamore::utils::apply_attribute(__el, ::std::clone::Clone::clone(&name), value);
                    }
                });
                (None, true)
            }
        }
    }

    fn component(&mut self, tag: &TagNode) -> TokenStream {
        // Add a DynMarker and set the component as a dyn value.
        self.dyn_values.push(impl_component(tag));
        quote! {
            ::sycamore::generic_node::TemplateShape::DynMarker
        }
    }

    fn text(&mut self, TextNode { value }: &TextNode) -> TokenStream {
        quote! {
            ::sycamore::generic_node::TemplateShape::Text(#value)
        }
    }

    fn dyn_marker(&mut self, DynNode { value }: &DynNode) -> TokenStream {
        let dyn_node = quote! {
            ::sycamore::view::View::new_dyn(move || ::sycamore::view::ToView::to_view(&(#value)))
        };
        self.dyn_values.push(dyn_node);

        quote! {
            ::sycamore::generic_node::TemplateShape::DynMarker
        }
    }
}

fn impl_component(node: &TagNode) -> TokenStream {
    let TagNode {
        ident,
        props,
        children,
        ..
    } = node;
    let ident = match ident {
        TagIdent::Path(path) => path,
        TagIdent::Hyphenated(_) => unreachable!("hyphenated tags are not components"),
    };

    let plain = props
        .iter()
        .filter_map(|prop| match &prop.ty {
            PropType::Plain { ident } => Some((ident, prop.value.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let plain_names = plain.iter().map(|(ident, _)| ident);
    let plain_values = plain.iter().map(|(_, value)| value);

    let attributes = props.iter().filter_map(|prop| match &prop.ty {
        PropType::Directive { dir, ident } => Some((dir, ident, prop.value.clone())),
        _ => None,
    });
    let attribute_entries_quoted = attributes
        .map(|(dir, name, value)| {
            let value = to_attribute_value(dir, name, &value)?;
            let name_str = if dir == "attr" {
                name.to_string()
            } else {
                format!("{dir}:{name}")
            };
            Ok(quote! {
                attributes.insert(::std::borrow::Cow::Borrowed(#name_str), #value)
            })
        })
        .collect::<Result<Vec<_>, syn::Error>>()
        .map_err(|err| err.to_compile_error());
    let attributes_quoted = if let Ok(attributes) = attribute_entries_quoted {
        if !attributes.is_empty() {
            quote! {
                .attributes({
                    let mut attributes = ::std::collections::HashMap::default();
                    #(#attributes;)*
                    ::sycamore::component::Attributes::new(attributes)
                })
            }
        } else {
            quote!()
        }
    } else {
        quote!()
    };

    let children_quoted = if children.0.is_empty() {
        quote! {}
    } else {
        let codegen = Codegen {};
        let children = codegen.root(children);
        quote! {
            .children(
                ::sycamore::component::Children::new(move || {
                    #children
                })
            )
        }
    };
    quote! {{
        let __component = &#ident; // We do this to make sure the compiler can infer the value for `<G>`.
        ::sycamore::component::component_scope(move || ::sycamore::component::Component::create(
            __component,
            ::sycamore::component::element_like_component_builder(__component)
                #(.#plain_names(#plain_values))*
                #children_quoted
                #attributes_quoted
                .build()
        ))
    }}
}

fn to_attribute_value(dir: &Ident, ident: &Ident, value: &Expr) -> Result<TokenStream, syn::Error> {
    match dir.to_string().as_str() {
        "on" => {
            let event = ident.to_string();

            Ok(quote!(::sycamore::component::AttributeValue::Event(
                #event,
                ::sycamore::utils::erase_handler::<::sycamore::web::html::ev::#ident, _, _>(#value)
            )))
        }
        "prop" => Ok(quote!(::sycamore::component::AttributeValue::Property(#ident, #value))),
        "bind" => match ident.to_string().as_str() {
            "value" => {
                Ok(quote!(::sycamore::component::AttributeValue::BindString("value", #value)))
            }
            "valueAsNumber" => Ok(
                quote!(::sycamore::component::AttributeValue::BindNumber("valueAsNumber", #value)),
            ),
            "checked" => {
                Ok(quote!(::sycamore::component::AttributeValue::BindBool("checked", #value)))
            }
            _ => Err(syn::Error::new(
                ident.span(),
                format!("property `{}` is not supported with `bind:`", ident),
            )),
        },
        "attr" => {
            if ident == "ref" {
                Ok(quote!(::sycamore::component::AttributeValue::Ref(#value)))
            } else if ident == "dangerously_set_inner_html" {
                if matches!(value, Expr::Lit(_)) {
                    Ok(
                        quote!(::sycamore::component::AttributeValue::DangerouslySetInnerHtml(#value.to_string())),
                    )
                } else {
                    Ok(
                        quote!(::sycamore::component::AttributeValue::DynamicDangerouslySetInnerHtml(Box::new(#value))),
                    )
                }
            } else if is_bool_attr(&ident.to_string()) {
                if matches!(value, Expr::Lit(_)) {
                    Ok(quote!(::sycamore::component::AttributeValue::Bool(#value)))
                } else {
                    Ok(quote!(::sycamore::component::AttributeValue::DynamicBool(
                        Box::new(move || #value)
                    )))
                }
            } else if matches!(value, Expr::Lit(_)) {
                Ok(quote!(::sycamore::component::AttributeValue::Str(#value)))
            } else {
                Ok(quote!(::sycamore::component::AttributeValue::DynamicStr(
                    Box::new(move || ::std::string::ToString::to_string(#value))
                )))
            }
        }
        _ => Err(syn::Error::new_spanned(
            ident,
            format!("unknown directive `{}`", ident),
        )),
    }
}

pub fn is_bool_attr(name: &str) -> bool {
    // Boolean attributes list from the WHATWG attributes table:
    // https://html.spec.whatwg.org/multipage/indices.html#attributes-3
    static BOOLEAN_ATTRIBUTES_SET: Lazy<HashSet<&str>> = Lazy::new(|| {
        vec![
            "allowfullscreen",
            "async",
            "autofocus",
            "autoplay",
            "checked",
            "controls",
            "default",
            "defer",
            "disabled",
            "formnovalidate",
            "hidden",
            "inert",
            "ismap",
            "itemscope",
            "loop",
            "multiple",
            "muted",
            "nomodule",
            "novalidate",
            "open",
            "playsinline",
            "readonly",
            "required",
            "reversed",
            "selected",
        ]
        .into_iter()
        .collect()
    });
    BOOLEAN_ATTRIBUTES_SET.contains(name)
}

pub fn is_void_element(name: &str) -> bool {
    static VOID_ELEMENTS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
        vec![
            "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
            "source", "track", "wbr", "command", "keygen", "menuitem",
        ]
        .into_iter()
        .collect()
    });
    VOID_ELEMENTS.contains(name)
}

pub fn is_component(ident: &TagIdent) -> bool {
    match ident {
        TagIdent::Path(path) => {
            path.get_ident().is_none()
                || path
                    .get_ident()
                    .unwrap()
                    .to_string()
                    .chars()
                    .next()
                    .unwrap()
                    .is_ascii_uppercase()
        }
        // A hyphenated tag is always a custom-element and therefore never a component.
        TagIdent::Hyphenated(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_component() {
        assert!(!is_component(&TagIdent::Path(syn::parse_quote! { div })));
        assert!(!is_component(&TagIdent::Path(syn::parse_quote! { a })));
        assert!(!is_component(&TagIdent::Hyphenated(
            "custom-element".to_string()
        )));

        assert!(is_component(&TagIdent::Path(
            syn::parse_quote! { MyComponent }
        )));
        assert!(is_component(&TagIdent::Path(
            syn::parse_quote! { not_elements::div }
        )));
    }
}
