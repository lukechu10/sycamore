//! Codegen for `view!` macro.
//!
//! Implementation note: We are not using the `quote::ToTokens` trait because we need to pass
//! additional information to the codegen such as which mode (Client, Hydrate, SSR), etc...

use proc_macro2::TokenStream;
use quote::{ToTokens, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Expr, Pat};

use crate::ir::{Attribute, DynNode, Node, Root, TagIdent, TagNode, TextNode};

pub fn expand_root(root: &Root) -> TokenStream {
    let quoted = match &root.0[..] {
        [] => quote! {
            ().build()
        },
        [node] => {
            let node = expand_node(node);
            quote_spanned! { node.span()=> #node.build() }
        }
        nodes => {
            let nodes = into_pair_stack(nodes.iter().map(expand_node));
            quote_spanned! { nodes.span()=>
                #nodes.build()
            }
        }
    };
    quote! {{
        use ::sycamore::rt::Build as _;
        #quoted
    }}
}

/// Generate a `View` from a `Node`.
pub fn expand_node(node: &Node) -> TokenStream {
    match node {
        Node::Tag(tag) => expand_component(tag),
        Node::Text(TextNode { value, format_args }) => match format_args {
            Some(args) => quote_spanned! { value.span()=>
                (move || ::std::format!(#value #(, #args)*)).build()
            },
            None => quote_spanned! { value.span()=> #value.build() },
        },
        Node::Dyn(DynNode { value }) => {
            if is_dyn(value) {
                quote_spanned! { value.span()=> (move || #value.build()).build() }
            } else {
                value.to_token_stream()
            }
        }
        Node::If(if_node) => {
            let cond = &if_node.cond;
            let is_dynamic = is_dyn(cond);

            let if_root = expand_root(&if_node.then);
            let else_root = if_node
                .else_branch
                .as_ref()
                .map(expand_root)
                .unwrap_or_else(|| quote_spanned! { cond.span()=> ().build() });

            if is_dynamic {
                quote_spanned! { cond.span()=>
                    (move || {
                        if #cond {
                            #if_root
                        } else {
                            #else_root
                        }
                    }).build()
                }
            } else {
                quote_spanned! { cond.span()=>
                    if #cond {
                        #if_root
                    } else {
                        #else_root
                    }
                }
            }
        }
        Node::Match(match_node) => {
            let match_expr = &match_node.expr;
            let is_dynamic =
                is_dyn(match_expr) || match_node.arms.iter().any(|arm| is_dyn_pattern(&arm.pat));

            let arms = match_node.arms.iter().map(|arm| {
                let pat = &arm.pat;
                let arm_root = expand_root(&arm.body);
                quote_spanned! { arm.pat.span()=>
                    #pat => #arm_root
                }
            });

            if is_dynamic {
                quote_spanned! { match_expr.span()=>
                    (move || {
                        match #match_expr {
                            #(#arms),*
                        }
                    }).build()
                }
            } else {
                quote_spanned! { match_expr.span()=>
                    match #match_expr {
                        #(#arms),*
                    }
                }
            }
        }
    }
}

pub fn expand_attribute(attr: &Attribute) -> TokenStream {
    match attr {
        Attribute::Plain { ident, value } => {
            quote_spanned! {ident.span()=> .#ident(#value) }
        }
        Attribute::PlainHyphenated { ident, value } => {
            quote_spanned! {ident.span()=> .attr(#ident, #value) }
        }
        Attribute::PlainQuoted { ident, value } => {
            quote_spanned! {ident.span()=> .attr(#ident, #value) }
        }
        Attribute::FnCall { ident, args } => {
            quote_spanned! {ident.span()=> .#ident(#args) }
        }
        Attribute::FnCallAssign { ident, args, value } => {
            quote_spanned! {ident.span()=> .#ident(#args, #value) }
        }
        Attribute::Directive { dir, ident, value } => {
            quote_spanned! {dir.span()=>
                .#dir(move |__tmp| __tmp.#ident(#value))
            }
        }
        Attribute::Spread { value } => quote! { .spread(#value) },
    }
}

pub fn expand_component(
    TagNode {
        ident,
        args,
        attributes,
        children,
    }: &TagNode,
) -> TokenStream {
    let constructor = match ident {
        TagIdent::Path(path) => quote_spanned! {path.span()=> #path(#(#args),*) },
        TagIdent::Hyphenated(name) => {
            quote_spanned! {ident.span()=> ::sycamore::rt::custom_element(#name) }
        }
    };

    let attributes_quoted = attributes.iter().map(expand_attribute);

    let children_quoted = if children.0.is_empty() {
        quote! {}
    } else {
        let children = into_pair_stack(children.0.iter().map(expand_node));
        quote_spanned! {ident.span()=> .children(#children) }
    };

    let build = quote_spanned!(ident.span()=> .build());

    quote! {{
        #constructor
            #(#attributes_quoted)*
            #children_quoted
            #build
    }}
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
                || m.arms
                    .iter()
                    .any(|a: &syn::Arm| is_dyn_pattern(&a.pat) || is_dyn(&a.body))
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

        Pat::Guard(g) => is_dyn_pattern(&g.pat) || is_dyn(&g.guard),
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

/// Convert an iterator of `TokenStream` into a `TokenStream` that is a stack of pairs, e.g.:
/// ```text
/// a, b, c -> (a, (b, c))
/// ```
fn into_pair_stack(iter: impl DoubleEndedIterator<Item = TokenStream>) -> TokenStream {
    let mut iter = iter.rev();
    let first = match iter.next() {
        Some(first) => first,
        None => return quote! { () },
    };
    iter.fold(
        first,
        |acc, item| quote_spanned! { item.span()=> (#item, #acc) },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_arguments_and_attributes_expand_separately() {
        let tag: TagNode = syn::parse2(quote! {
            gtk::Label::new(a = b, { log(); Some("initial label") }, make_value(),;
                prop:set_selectable = true,
                prop("abc", 123),
            )
        })
        .unwrap();
        let expected = quote! {{
            gtk::Label::new(a = b, { log(); Some("initial label") }, make_value())
                .prop(move |__tmp| __tmp.set_selectable(true))
                .prop("abc", 123)
                .build()
        }};
        assert_eq!(expand_component(&tag).to_string(), expected.to_string());
    }

    #[test]
    fn constructor_separator_allows_either_side_to_be_empty() {
        let tag: TagNode = syn::parse2(quote! { gtk::Label::new(None;) }).unwrap();
        let expected = quote! {{ gtk::Label::new(None).build() }};
        assert_eq!(expand_component(&tag).to_string(), expected.to_string());

        let tag: TagNode = syn::parse2(quote! { button(; class = "abc") }).unwrap();
        let expected = quote! {{ button().class("abc").build() }};
        assert_eq!(expand_component(&tag).to_string(), expected.to_string());
    }

    #[test]
    fn no_separator_preserves_attribute_syntax() {
        let tag: TagNode =
            syn::parse2(quote! { button(class = { log(); "abc" }, prop("abc", 123)) }).unwrap();
        let expected = quote! {{ button().class({ log(); "abc" }).prop("abc", 123).build() }};
        assert_eq!(expand_component(&tag).to_string(), expected.to_string());
    }

    #[test]
    fn custom_element_tags_reject_constructor_arguments() {
        let Err(err) = syn::parse2::<TagNode>(quote! { my-widget(value;) }) else {
            panic!("expected a parse error");
        };
        assert_eq!(
            err.to_string(),
            "custom element tags do not take constructor arguments"
        );
    }

    #[test]
    fn attribute_expressions_are_passed_directly() {
        for value in [quote! { signal.get() }, quote! { move || signal.get() }] {
            let value: Expr = syn::parse2(value).unwrap();
            let root: Root = syn::parse2(quote! {
                input(
                    value=#value,
                    data-value=#value,
                    "quoted-value"=#value,
                    prop("value")=#value,
                )
            })
            .unwrap();
            let Node::Tag(tag) = &root.0[0] else {
                panic!("expected element");
            };
            let actual: TokenStream = tag.attributes.iter().map(expand_attribute).collect();
            let expected = quote! {
                .value(#value)
                .attr("data-value", #value)
                .attr("quoted-value", #value)
                .prop("value", #value)
            };
            assert_eq!(actual.to_string(), expected.to_string());
        }
    }
}
