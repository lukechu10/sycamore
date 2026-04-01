//! Codegen for `view!` macro.
//!
//! Implementation note: We are not using the `quote::ToTokens` trait because we need to pass
//! additional information to the codegen such as which mode (Client, Hydrate, SSR), etc...

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Pat};

use crate::ir::{DynNode, Node, Prop, PropType, Root, TagIdent, TagNode, TextNode};

pub struct Codegen {
    // TODO: configure mode: Client, Hydrate, SSR
}

impl Codegen {
    pub fn root(&self, root: &Root) -> TokenStream {
        match &root.0[..] {
            [] => quote! {
                ::sycamore::rt::View::new()
            },
            [node] => self.node(node),
            nodes => {
                let nodes = nodes.iter().map(|node| self.node(node));
                quote! {
                    ::std::convert::Into::<::sycamore::rt::View>::into(::std::vec![#(#nodes),*])
                }
            }
        }
    }

    /// Generate a `View` from a `Node`.
    pub fn node(&self, node: &Node) -> TokenStream {
        match node {
            Node::Tag(tag) => {
                if is_component(&tag.ident) {
                    self.component(tag)
                } else {
                    self.element(tag)
                }
            }
            Node::Text(TextNode { value }) => quote! {
                ::std::convert::Into::<::sycamore::rt::View>::into(#value)
            },
            Node::Dyn(DynNode { value }) => {
                let is_dynamic = is_dyn(value);
                if is_dynamic {
                    quote! {
                        ::sycamore::rt::View::from_dynamic(
                            move || ::std::convert::Into::<::sycamore::rt::View>::into(#value)
                        )
                    }
                } else {
                    quote! {
                        ::std::convert::Into::<::sycamore::rt::View>::into(#value)
                    }
                }
            }
        }
    }

    pub fn element(&self, element: &TagNode) -> TokenStream {
        let TagNode {
            ident,
            props,
            children,
        } = element;

        let attributes = props.iter().map(|attr| self.attribute(attr));

        let children = children
            .0
            .iter()
            .map(|child| self.node(child))
            .collect::<Vec<_>>();

        match ident {
            TagIdent::Path(tag) => {
                assert!(tag.get_ident().is_some(), "elements must be an ident");
                quote! {
                    ::sycamore::rt::View::from(
                        ::sycamore::rt::tags::#tag().children(::std::vec![#(#children),*])#(#attributes)*
                    )
                }
            }
            TagIdent::Hyphenated(tag) => quote! {
                ::sycamore::rt::View::from(
                    ::sycamore::rt::custom_element(#tag).children(::std::vec![#(#children),*])#(#attributes)*
                )
            },
        }
    }

    pub fn attribute(&self, attr: &Prop) -> TokenStream {
        let value = &attr.value;
        let is_dynamic = is_dyn(value);
        let dyn_value = if is_dynamic {
            quote! { move || #value }
        } else {
            quote! { #value }
        };
        match &attr.ty {
            PropType::Plain { ident } => {
                quote! { .#ident(#dyn_value) }
            }
            PropType::PlainHyphenated { ident } => {
                quote! { .attr(#ident, #dyn_value) }
            }
            PropType::PlainQuoted { ident } => {
                quote! { .attr(#ident, #dyn_value) }
            }
            PropType::Directive { dir, ident } => match dir.to_string().as_str() {
                "on" => quote! { .on(::sycamore::rt::events::#ident, #value) },
                "prop" => {
                    let ident = ident.to_string();
                    quote! { .prop(#ident, #dyn_value) }
                }
                "bind" => quote! { .bind(::sycamore::rt::bind::#ident, #value) },
                _ => syn::Error::new(dir.span(), format!("unknown directive `{dir}`"))
                    .to_compile_error(),
            },
            PropType::Ref => quote! { .r#ref(#value) },
            PropType::Spread => quote! { .spread(#value) },
        }
    }

    pub fn component(
        &self,
        TagNode {
            ident,
            props,
            children,
        }: &TagNode,
    ) -> TokenStream {
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

        let other_props = props
            .iter()
            .filter(|prop| !matches!(&prop.ty, PropType::Plain { .. }))
            .collect::<Vec<_>>();
        let other_attributes = other_props.iter().map(|prop| self.attribute(prop));

        let children_quoted = if children.0.is_empty() {
            quote! {}
        } else {
            let codegen = Codegen {};
            let children = codegen.root(children);
            quote! {
                .children(
                    ::sycamore::rt::Children::new(move || {
                        #children
                    })
                )
            }
        };
        quote! {{
            let __component = &#ident; // We do this to make sure the compiler can infer the value for `<G>`.
            ::sycamore::rt::component_scope(move || ::sycamore::rt::Component::create(
                __component,
                ::sycamore::rt::element_like_component_builder(__component)
                    #(.#plain_names(#plain_values))*
                    #(#other_attributes)*
                    #children_quoted
                    .build()
            ))
        }}
    }
}

fn is_component(ident: &TagIdent) -> bool {
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

fn is_dyn(ex: &Expr) -> bool {
    match ex {
        Expr::Lit(_) | Expr::Closure(_) | Expr::Path(_) => false,

        Expr::Field(f) => is_dyn(&f.base),
        Expr::Paren(p) => is_dyn(&p.expr),
        Expr::Group(g) => is_dyn(&g.expr),
        Expr::Tuple(t) => t.elems.iter().any(is_dyn),
        Expr::Array(a) => a.elems.iter().any(is_dyn),
        Expr::Repeat(r) => is_dyn(&r.expr) || is_dyn(&r.len),
        Expr::Struct(s) => s.fields.iter().any(|fv: &syn::FieldValue| is_dyn(&fv.expr)),

        Expr::Cast(c) => is_dyn(&c.expr),
        Expr::Macro(m) => is_dyn_macro(&m.mac),
        Expr::Block(b) => is_dyn_block(&b.block),
        Expr::Const(_const_block) => false,

        Expr::Loop(l) => is_dyn_block(&l.body),
        Expr::While(w) => is_dyn(&w.cond) || is_dyn_block(&w.body),
        Expr::ForLoop(f) => is_dyn_pattern(&f.pat) || is_dyn(&f.expr) || is_dyn_block(&f.body),
        Expr::Break(_) | Expr::Continue(_) => false,

        Expr::Let(e) => is_dyn_pattern(&e.pat) || is_dyn(&e.expr),

        Expr::Match(m) => {
            is_dyn(&m.expr)
                || m.arms.iter().any(|a: &syn::Arm| {
                    is_dyn_pattern(&a.pat)
                        || a.guard.as_ref().is_some_and(|(_, g_expr)| is_dyn(g_expr))
                        || is_dyn(&a.body)
                })
        }

        Expr::If(i) => {
            is_dyn(&i.cond)
                || is_dyn_block(&i.then_branch)
                || i.else_branch.as_ref().is_some_and(|(_, e)| is_dyn(e))
        }

        Expr::Unary(u) => is_dyn(&u.expr),
        Expr::Binary(b) => is_dyn(&b.left) || is_dyn(&b.right),
        Expr::Index(i) => is_dyn(&i.expr) || is_dyn(&i.index),
        Expr::Range(r) => {
            r.start.as_deref().is_some_and(is_dyn) || r.end.as_deref().is_some_and(is_dyn)
        }

        _ => true,
    }
}

fn is_dyn_pattern(pat: &Pat) -> bool {
    match pat {
        Pat::Wild(_) | Pat::Lit(_) | Pat::Path(_) | Pat::Rest(_) | Pat::Type(_) | Pat::Const(_) => {
            false
        }

        Pat::Paren(p) => is_dyn_pattern(&p.pat),
        Pat::Or(o) => o.cases.iter().any(is_dyn_pattern),
        Pat::Tuple(t) => t.elems.iter().any(is_dyn_pattern),
        Pat::TupleStruct(s) => s.elems.iter().any(is_dyn_pattern),
        Pat::Slice(s) => s.elems.iter().any(is_dyn_pattern),
        Pat::Range(r) => {
            r.start.as_deref().is_some_and(is_dyn) || r.end.as_deref().is_some_and(is_dyn)
        }

        Pat::Reference(r) => r.mutability.is_some(),
        Pat::Ident(id) => {
            (id.by_ref.is_some() && id.mutability.is_some())
                || id
                    .subpat
                    .as_ref()
                    .is_some_and(|(_, pat)| is_dyn_pattern(pat))
        }

        Pat::Struct(s) => s
            .fields
            .iter()
            .any(|fp: &syn::FieldPat| is_dyn_pattern(&fp.pat)),

        // syn::Pat is non-exhaustive
        _ => true,
    }
}

fn is_dyn_macro(m: &syn::Macro) -> bool {
    // Bodies of nested inner view! macros will be checked for dynamic
    // parts when their own codegen is run.

    m.path.get_ident().is_none_or(|ident| ident != "view")
}

fn is_dyn_block(block: &syn::Block) -> bool {
    block.stmts.iter().any(|s: &syn::Stmt| match s {
        syn::Stmt::Expr(ex, _) => is_dyn(ex),
        syn::Stmt::Macro(m) => is_dyn_macro(&m.mac),
        syn::Stmt::Local(loc) => {
            is_dyn_pattern(&loc.pat)
                || loc.init.as_ref().is_some_and(|i| {
                    is_dyn(&i.expr) || i.diverge.as_ref().is_some_and(|(_, ex)| is_dyn(ex))
                })
        }
        syn::Stmt::Item(_) => false,
    })
}
