//! Codegen for `view!` macro.
//!
//! Note: we are not using the `ToTokens` trait from `quote` because we need to keep track
//! of some internal state during the entire codegen.

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Expr, ExprLit, Ident, Lit};

use crate::view::ir::*;

/// A struct for keeping track of the state when emitting Rust code.
pub struct Codegen {
    pub cx: Ident,
}

impl Codegen {
    pub fn view_root(&self, view_root: &ViewRoot) -> TokenStream {
        match &view_root.0[..] {
            [] => quote! {
                ::sycamore::view::View::empty()
            },
            [node] => self.view_node(node),
            nodes => {
                let append_nodes: TokenStream = nodes
                    .iter()
                    .map(|node| {
                        let quoted = self.view_node(node);
                        quote! { #quoted, }
                    })
                    .collect();
                quote! {
                    ::sycamore::view::View::new_fragment({
                        ::std::vec![
                            #append_nodes
                        ]
                    })
                }
            }
        }
    }

    pub fn view_node(&self, view_node: &ViewNode) -> TokenStream {
        let cx = &self.cx;
        match view_node {
            ViewNode::Element(elem) => {
                let elem = self.element(elem);
                quote! {
                    ::sycamore::view::View::new_node(#elem)
                }
            }
            ViewNode::Component(comp) => self.component(comp),
            ViewNode::Text(Text { value }) => quote! {
                ::sycamore::view::View::new_node(::sycamore::generic_node::GenericNode::text_node(#value))
            },
            ViewNode::Dyn(d @ Dyn { value }) => {
                let needs_cx = d.needs_cx(&cx.to_string());
                match needs_cx {
                    true => quote! {
                        ::sycamore::view::View::new_dyn_scoped(#cx, move |#cx|
                            ::sycamore::view::IntoView::create(&(#value))
                        )
                    },
                    false => quote! {
                        ::sycamore::view::View::new_dyn(#cx, move ||
                            ::sycamore::view::IntoView::create(&(#value))
                        )
                    },
                }
            }
        }
    }

    pub fn element(&self, elem: &Element) -> TokenStream {
        let cx = &self.cx;
        let Element {
            tag,
            attrs,
            children,
        } = elem;

        let quote_tag = match tag {
            ElementTag::Builtin(id) => quote! {
                let __el = ::sycamore::generic_node::GenericNode::element::<::sycamore::web::html::#id>();
            },
            ElementTag::Custom(tag_s) => quote! {
                let __el = ::sycamore::generic_node::GenericNode::element_from_tag(#tag_s);
            },
        };

        let quote_attrs: TokenStream = attrs.iter().map(|attr| self.attribute(attr)).collect();

        let quote_children = {
            let multi = children.len() >= 2;
            let mut children = children.iter().peekable();
            let mut quoted = TokenStream::new();
            while let Some(child) = children.next() {
                let is_dyn = child.is_dynamic();
                if is_dyn {
                    let codegen_ssr_markers = cfg!(feature = "ssr") && multi;
                    let mut marker_is_some = true;
                    let marker = if let Some(ViewNode::Element(elem)) =
                        children.next_if(|x| matches!(x, ViewNode::Element(_)))
                    {
                        let elem = self.element(elem);
                        quote! {
                            let __marker = #elem;
                            ::sycamore::generic_node::GenericNode::append_child(&__el, &__marker);
                            let __marker = ::std::option::Option::Some(&__marker);
                        }
                    } else if let Some(ViewNode::Text(Text { value })) =
                        children.next_if(|x| matches!(x, ViewNode::Text(_)))
                    {
                        quote! {
                            let __marker = ::sycamore::generic_node::GenericNode::text_node(#value);
                            ::sycamore::generic_node::GenericNode::append_child(&__el, &__marker);
                            let __marker = ::std::option::Option::Some(&__marker);
                        }
                    } else if children.peek().is_none() {
                        marker_is_some = false;
                        quote! {
                            let __marker = ::std::option::Option::None;
                        }
                    } else {
                        quote! {
                            let __marker = ::sycamore::generic_node::GenericNode::marker();
                            ::sycamore::generic_node::GenericNode::append_child(&__el, &__marker);
                            let __marker = ::std::option::Option::Some(&__marker);
                        }
                    };
                    let marker_or_none = marker_is_some.then(|| marker.clone()).unwrap_or_default();

                    let initial = quote! { ::sycamore::utils::initial_node(&__el) };
                    let ssr_markers = quote! {
                        ::sycamore::generic_node::GenericNode::append_child(
                            &__el,
                            &::sycamore::generic_node::GenericNode::marker_with_text("#"),
                        );
                        let __end_marker = ::sycamore::generic_node::GenericNode::marker_with_text("/");
                        ::sycamore::generic_node::GenericNode::append_child(&__el, &__end_marker);
                    };

                    quoted.extend(match child {
                        ViewNode::Component(comp) => {
                            let comp = self.component(comp);
                            let quoted = quote! {
                                #marker
                                ::sycamore::utils::render::insert(#cx, &__el, __comp, __initial, __marker, #multi);
                            };
                            codegen_ssr_markers.then(|| quote! {
                                let __comp = #comp;
                                let __initial = #initial;
                                if ::std::any::Any::type_id(&__el) == ::std::any::TypeId::of::<::sycamore::web::SsrNode>() {
                                    #ssr_markers
                                    ::sycamore::utils::render::insert(#cx, &__el, __comp, __initial, Some(&__end_marker), #multi);
                                    #marker_or_none
                                } else { #quoted }
                            }).unwrap_or(quote! {
                                let __comp = #comp;
                                let __initial = #initial;
                                #quoted
                            })
                        }
                        ViewNode::Dyn(d @ Dyn { value}) => {
                            let needs_cx = d.needs_cx(&self.cx.to_string());
                            let view_quoted = match needs_cx {
                                true => quote! {
                                    ::sycamore::view::View::new_dyn_scoped(#cx, move |#cx|
                                        ::sycamore::view::IntoView::create(&(#value))
                                    )
                                },
                                false => quote! {
                                    ::sycamore::view::View::new_dyn(#cx, move ||
                                        ::sycamore::view::IntoView::create(&(#value))
                                    )
                                }
                            };
                            let quoted = quote! {
                                #marker
                                ::sycamore::utils::render::insert(#cx, &__el, __view, __initial, __marker, #multi);
                            };
                            codegen_ssr_markers.then(|| quote! {
                                let __view = #view_quoted;
                                let __initial = #initial;
                                if ::std::any::Any::type_id(&__el) == ::std::any::TypeId::of::<::sycamore::web::SsrNode>() {
                                    #ssr_markers
                                    ::sycamore::utils::render::insert(
                                        #cx, &__el, __view, __initial, Some(&__end_marker), #multi
                                    );
                                    #marker_or_none
                                } else { #quoted }
                            }).unwrap_or(quote! {
                                let __view = #view_quoted;
                                let __initial = #initial;
                                #quoted
                            })
                        },
                        _ => unreachable!("only component and dyn node can be dynamic"),
                    });

                    // Do not perform non dynamic codegen.
                    continue;
                }
                match child {
                    ViewNode::Element(elem) => quoted.extend({
                        let elem = self.element(elem);
                        quote! {
                            ::sycamore::generic_node::GenericNode::append_child(&__el, &#elem);
                        }
                    }),
                    ViewNode::Component(_) => unreachable!("component is always dynamic"),
                    ViewNode::Text(Text { value }) => {
                        let intern = quote! {
                            // Since this is static text, intern it as it will likely be constructed many times.
                            #[cfg(target_arch = "wasm32")]
                            ::sycamore::rt::intern(#value);
                        };
                        quoted.extend(match multi {
                            true => quote! {
                                #intern
                                ::sycamore::generic_node::GenericNode::append_child(
                                    &__el,
                                    &::sycamore::generic_node::GenericNode::text_node(#value),
                                );
                            },
                            // Only one child, directly set innerText instead of creating a text node.
                            false => quote! {
                                #intern
                                ::sycamore::generic_node::GenericNode::update_inner_text(&__el, #value);
                            },
                        });
                    }
                    ViewNode::Dyn(Dyn { value }) => quoted.extend(quote! {
                        ::sycamore::utils::render::insert(#cx, &__el,
                            ::sycamore::view::IntoView::create(&(#value)),
                            None, None, #multi
                        );
                    }),
                }
            }
            quoted
        };

        quote! {{
            #quote_tag
            #quote_attrs
            #quote_children
            __el
        }}
    }

    pub fn attribute(&self, attr: &Attribute) -> TokenStream {
        let cx = &self.cx;
        let mut tokens = TokenStream::new();
        let expr = &attr.value;

        let is_dynamic = !matches!(expr, Expr::Lit(ExprLit { .. }));

        match &attr.ty {
            AttributeType::Str { name } => {
                let name = name.to_string();
                // Use `set_class_name` instead of `set_attribute` for better performance.
                let is_class = name == "class";
                let quoted_text = if let Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) = expr
                {
                    // Since this is static text, intern it as it will likely be constructed many
                    // times.
                    quote! {
                        if ::std::cfg!(target_arch = "wasm32") {
                            ::sycamore::rt::intern(#text)
                        } else {
                            #text
                        }
                    }
                } else {
                    quote! {
                        &::std::string::ToString::to_string(&#expr)
                    }
                };
                let quoted_set_attribute = if is_class {
                    quote! {
                        ::sycamore::generic_node::GenericNode::set_class_name(&__el, #quoted_text);
                    }
                } else {
                    quote! {
                        ::sycamore::generic_node::GenericNode::set_attribute(&__el, #name, #quoted_text);
                    }
                };

                if is_dynamic {
                    tokens.extend(quote! {
                        ::sycamore::reactive::create_effect(#cx, {
                            let __el = ::std::clone::Clone::clone(&__el);
                            move || { #quoted_set_attribute }
                        });
                    });
                } else {
                    tokens.extend(quote! { #quoted_set_attribute });
                };
            }
            AttributeType::Bool { name } => {
                let name = name.to_string();
                let quoted_set_attribute = quote! {
                    if #expr {
                        ::sycamore::generic_node::GenericNode::set_attribute(&__el, #name, "");
                    } else {
                        ::sycamore::generic_node::GenericNode::remove_attribute(&__el, #name);
                    }
                };

                if is_dynamic {
                    tokens.extend(quote! {
                        ::sycamore::reactive::create_effect(#cx, {
                            let __el = ::std::clone::Clone::clone(&__el);
                            move || {
                                #quoted_set_attribute
                            }
                        });
                    });
                } else {
                    tokens.extend(quote! {
                        #quoted_set_attribute
                    });
                };
            }
            AttributeType::DangerouslySetInnerHtml => {
                if is_dynamic {
                    tokens.extend(quote! {
                        ::sycamore::reactive::create_effect(#cx, {
                            let __el = ::std::clone::Clone::clone(&__el);
                            move || {
                                ::sycamore::generic_node::GenericNode::dangerously_set_inner_html(
                                    &__el,
                                    #expr,
                                );
                            }
                        });
                    });
                } else {
                    tokens.extend(quote! {
                        ::sycamore::generic_node::GenericNode::dangerously_set_inner_html(
                            &__el,
                            #expr,
                        );
                    });
                };
            }
            AttributeType::Event { event } => {
                tokens.extend(quote! {
                    ::sycamore::generic_node::GenericNode::event(
                        &__el,
                        #cx,
                        #event,
                        #expr,
                    );
                });
            }
            AttributeType::Property { prop } => {
                let set_property = quote! {
                    ::sycamore::generic_node::GenericNode::set_property(
                        &__el,
                        #prop,
                        &::std::convert::Into::<::sycamore::rt::JsValue>::into(#expr)
                    );
                };
                if is_dynamic {
                    tokens.extend(quote! {
                        ::sycamore::reactive::create_effect(#cx, {
                            let __el = ::std::clone::Clone::clone(&__el);
                            move || { #set_property }
                        });
                    });
                } else {
                    tokens.extend(set_property);
                }
            }
            AttributeType::Bind { prop } => {
                #[derive(Clone, Copy)]
                enum JsPropertyType {
                    Bool,
                    String,
                    Number,
                }

                let (event_name, property_ty) = match prop.as_str() {
                    "value" => ("input", JsPropertyType::String),
                    "valueAsNumber" => ("input", JsPropertyType::Number),
                    "checked" => ("change", JsPropertyType::Bool),
                    _ => {
                        tokens.extend(
                            syn::Error::new(
                                prop.span(),
                                format!("property `{}` is not supported with `bind:`", prop),
                            )
                            .to_compile_error(),
                        );
                        return tokens;
                    }
                };

                let convert_into_jsvalue_fn = match property_ty {
                    JsPropertyType::Bool => quote! {
                        ::sycamore::rt::JsValue::from_bool(
                            *::sycamore::reactive::ReadSignal::get(&#expr)
                        )
                    },
                    JsPropertyType::Number => quote! {
                        ::sycamore::rt::JsValue::from_f64(
                            *::sycamore::reactive::ReadSignal::get(&#expr)
                        )
                    },
                    JsPropertyType::String => quote! {
                        ::sycamore::rt::JsValue::from_str(
                            &::std::string::ToString::to_string(
                                &::sycamore::reactive::ReadSignal::get(&#expr),
                            )
                        )
                    },
                };

                let event_target_prop = quote! {
                    ::sycamore::rt::Reflect::get(
                        &event.target().unwrap(),
                        &::std::convert::Into::<::sycamore::rt::JsValue>::into(#prop)
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

                tokens.extend(quote! {
                    #[cfg(target_arch = "wasm32")]
                    ::sycamore::reactive::create_effect(#cx, {
                        let __el = ::std::clone::Clone::clone(&__el);
                        let #expr = ::std::clone::Clone::clone(&#expr);
                        move ||::sycamore::generic_node::GenericNode::set_property(
                            &__el,
                            #prop,
                            &#convert_into_jsvalue_fn,
                        )
                    });
                    ::sycamore::generic_node::GenericNode::event(&__el, #cx, #event_name,
                    {
                        let #expr = ::std::clone::Clone::clone(&#expr);
                        ::std::boxed::Box::new(move |event: ::sycamore::rt::Event| {
                            ::sycamore::reactive::Signal::set(&#expr, #convert_from_jsvalue_fn);
                        })
                    },
                    );
                });
            }
            AttributeType::Ref => {
                tokens.extend(quote! {{
                    ::sycamore::noderef::NodeRef::set(&#expr, ::std::clone::Clone::clone(&__el));
                }});
            }
        }
        tokens
    }

    pub fn component(&self, comp: &Component) -> TokenStream {
        let cx = &self.cx;
        match comp {
            Component::Legacy(comp) => {
                let LegacyComponent { ident, args } = comp;
                quote! { ::sycamore::component::component_scope(move || #ident(#cx, #args)) }
            }
            Component::New(comp) => {
                let NewComponent {
                    ident,
                    props,
                    children,
                    ..
                } = comp;
                if props.is_empty()
                    && (children.is_none() || children.as_ref().unwrap().0.is_empty())
                {
                    quote! {
                       ::sycamore::component::component_scope(move || #ident(#cx))
                    }
                } else {
                    let name = props.iter().map(|x| &x.name);
                    let value = props.iter().map(|x| &x.value);
                    let children_quoted = children
                        .as_ref()
                        .map(|children| {
                            let children = self.view_root(children);
                            quote! {
                                .children(
                                    ::sycamore::component::Children::new(#cx, move |#cx| {
                                        #[allow(unused_variables)]
                                        let #cx: ::sycamore::reactive::BoundedScope = #cx;
                                        #children
                                    })
                                )
                            }
                        })
                        .unwrap_or_default();
                    quote! {{
                        let __component = &#ident; // We do this to make sure the compiler can infer the value for `<G>`.
                        ::sycamore::component::component_scope(move || __component(
                            #cx,
                            ::sycamore::component::element_like_component_builder(__component)
                                #(.#name(#value))*
                                #children_quoted
                                .build()
                        ))
                    }}
                }
            }
        }
    }
}
