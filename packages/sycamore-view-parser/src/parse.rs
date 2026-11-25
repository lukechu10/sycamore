//! Parse syntax for `view!` macro.

use std::collections::HashSet;

use proc_macro2::TokenTree;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::token::{Brace, Paren};
use syn::{Expr, Ident, LitStr, Result, Token, braced, parenthesized, token};

use crate::ir::*;

impl Parse for Root {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut children = Vec::new();

        while !input.is_empty() {
            children.push(input.parse()?);
        }

        Ok(Self(children))
    }
}

impl Node {
    fn peek_type(input: ParseStream) -> Option<NodeType> {
        let input = input.fork(); // do not affect original ParseStream

        if input.peek(LitStr) {
            Some(NodeType::Text)
        } else if input.peek(Paren) {
            Some(NodeType::Dyn)
        } else if input.peek(Token![if]) {
            Some(NodeType::If)
        } else if input.peek(Token![match]) {
            Some(NodeType::Match)
        } else if input.peek(Token![::]) || input.peek(Ident::peek_any) {
            Some(NodeType::Tag)
        } else {
            None
        }
    }
}

fn starts_slot(input: ParseStream) -> bool {
    input.peek(Token![:]) && !input.peek(Token![::])
}

impl Parse for Node {
    fn parse(input: ParseStream) -> Result<Self> {
        if starts_slot(input) {
            return Err(input.error("named slots must be direct children of a tag"));
        }
        let ty = match Self::peek_type(input) {
            Some(ty) => ty,
            None => return Err(input.error("expected a valid node")),
        };

        Ok(match ty {
            NodeType::Tag => Self::Tag(input.parse()?),
            NodeType::Text => Self::Text(input.parse()?),
            NodeType::Dyn => Self::Dyn(input.parse()?),
            NodeType::If => Self::If(input.parse()?),
            NodeType::Match => Self::Match(input.parse()?),
        })
    }
}

impl Parse for TagNode {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: TagIdent = input.parse()?;

        let has_paren = input.peek(token::Paren);

        let (args, attributes): (_, Vec<Attribute>) = if has_paren {
            let content;
            parenthesized!(content in input);

            // Look for a top-level separator without trying to interpret ambiguous syntax
            // such as `a = b` or `f(x)`. Nested groups are consumed as single token trees.
            let lookahead = content.fork();
            while !lookahead.is_empty() && !lookahead.peek(Token![;]) {
                lookahead.parse::<TokenTree>()?;
            }

            let mut args = Vec::new();
            if lookahead.peek(Token![;]) {
                if matches!(ident, TagIdent::Hyphenated(_)) && !content.peek(Token![;]) {
                    return Err(
                        content.error("custom element tags do not take constructor arguments")
                    );
                }
                while !content.peek(Token![;]) {
                    args.push(content.parse::<Expr>()?);
                    if !content.peek(Token![;]) {
                        content.parse::<Token![,]>()?;
                    }
                }
                content.parse::<Token![;]>()?;
            }

            let attributes = content
                .parse_terminated(Attribute::parse, Token![,])?
                .into_iter()
                .collect();
            (args, attributes)
        } else {
            (Vec::new(), Vec::new())
        };

        if !has_paren && !input.peek(Brace) {
            return Err(input.error("expected either `(` or `{` after element tag"));
        }

        let mut children = Vec::new();
        let mut slots = Vec::new();
        if input.peek(Brace) {
            let content;
            braced!(content in input);
            // Only used for duplicate detection. `slots` retains source order for codegen.
            let mut slot_names = HashSet::new();
            let mut children_slot = None;
            while !content.is_empty() {
                if starts_slot(&content) {
                    let slot: Slot = content.parse()?;
                    let name = slot.ident.unraw();
                    let directive = slot.directive.as_ref().map(Ident::unraw);
                    if !slot_names.insert((directive.clone(), name.clone())) {
                        return Err(syn::Error::new(slot.ident.span(), "duplicate named slot"));
                    }
                    if attributes.iter().any(|attr| match (attr, &directive) {
                        (
                            Attribute::Plain { ident, .. }
                            | Attribute::FnCall { ident, .. }
                            | Attribute::FnCallAssign { ident, .. },
                            None,
                        ) => ident.unraw() == name,
                        (Attribute::Directive { dir, ident, .. }, Some(directive)) => {
                            dir.unraw() == *directive && ident.unraw() == name
                        }
                        _ => false,
                    }) {
                        return Err(syn::Error::new(
                            slot.ident.span(),
                            "named slot conflicts with an attribute on this tag",
                        ));
                    }
                    if directive.is_none() && name == "children" {
                        children_slot = Some(slot.ident.span());
                    }
                    slots.push(slot);
                } else {
                    children.push(content.parse()?);
                }
            }
            if let Some(span) = children_slot.filter(|_| !children.is_empty()) {
                return Err(syn::Error::new(
                    span,
                    "the `children` slot conflicts with unnamed children",
                ));
            }
        }

        Ok(Self {
            ident,
            args,
            attributes,
            children: Root(children),
            slots,
        })
    }
}

impl Parse for Slot {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![:]>()?;
        let first = input.call(Ident::parse_any)?;
        let (directive, ident) = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            (Some(first), input.call(Ident::parse_any)?)
        } else {
            (None, first)
        };
        let content;
        braced!(content in input);
        Ok(Self {
            directive,
            ident,
            children: content.parse()?,
        })
    }
}

impl Parse for TagIdent {
    fn parse(input: ParseStream) -> Result<Self> {
        let is_hyphenated = input.peek2(Token![-]);
        if is_hyphenated {
            let mut segments: Vec<Ident> = vec![input.call(Ident::parse_any)?];
            while input.peek(Token![-]) {
                let _: Token![-] = input.parse()?;
                segments.push(input.parse()?);
            }
            let tag = segments
                .into_iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join("-");
            Ok(Self::Hyphenated(tag))
        } else {
            Ok(Self::Path(input.parse()?))
        }
    }
}

impl Parse for Attribute {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![..]) {
            let _2dot = input.parse::<Token![..]>()?;
            let value = input.parse()?;
            Ok(Self::Spread { value })
        } else if lookahead.peek(Ident::peek_any) {
            // Check if we are parsing a hyphenated attribute.
            if input.peek2(Token![-]) {
                let mut segments: Vec<Ident> = vec![input.call(Ident::parse_any)?];
                while input.peek(Token![-]) {
                    let _: Token![-] = input.parse()?;
                    segments.push(input.call(Ident::parse_any)?);
                }
                let ident = segments
                    .into_iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join("-");
                let _eq = input.parse::<Token![=]>()?;
                let value = input.parse()?;
                Ok(Self::PlainHyphenated { ident, value })
            }
            // Check if we are parsing a function call attribute.
            else if input.peek2(Paren) {
                let ident: Ident = input.call(Ident::parse_any)?;
                let content;
                parenthesized!(content in input);
                let args = content.parse_terminated(Expr::parse, Token![,])?;
                if input.peek(Token![=]) {
                    let _eq = input.parse::<Token![=]>()?;
                    let value = input.parse()?;
                    Ok(Self::FnCallAssign { ident, args, value })
                } else {
                    Ok(Self::FnCall { ident, args })
                }
            } else {
                let ident: Ident = input.call(Ident::parse_any)?;

                if input.peek(Token![:]) {
                    let _colon: Token![:] = input.parse()?;
                    let dir = ident;
                    let ident = input.call(Ident::parse_any)?;
                    let _eq = input.parse::<Token![=]>()?;
                    let value = input.parse()?;
                    Ok(Self::Directive { dir, ident, value })
                } else {
                    let _eq = input.parse::<Token![=]>()?;
                    let value = input.parse()?;
                    Ok(Self::Plain { ident, value })
                }
            }
        } else if lookahead.peek(LitStr) {
            let ident: String = <LitStr as Parse>::parse(input).map(|s| s.value())?;
            let _eq = input.parse::<Token![=]>()?;
            let value = input.parse()?;
            Ok(Self::PlainQuoted { ident, value })
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for TextNode {
    fn parse(input: ParseStream) -> Result<Self> {
        crate::interpolation::parse(input.parse()?)
    }
}

impl Parse for DynNode {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        Ok(Self {
            value: content.parse()?,
        })
    }
}

impl Parse for IfNode {
    fn parse(input: ParseStream) -> Result<Self> {
        let _if: Token![if] = input.parse()?;
        let cond = syn::Expr::parse_without_eager_brace(input)?;

        let then_content;
        braced!(then_content in input);
        let then = then_content.parse()?;

        let else_branch = if input.peek(Token![else]) {
            let _else: Token![else] = input.parse()?;
            // Check if the else branch is another if statement (else if)
            if input.peek(Token![if]) {
                let else_if_node: IfNode = input.parse()?;
                Some(Root(vec![Node::If(else_if_node)]))
            } else {
                let else_content;
                braced!(else_content in input);
                let else_branch = else_content.parse()?;
                Some(else_branch)
            }
        } else {
            None
        };

        Ok(Self {
            cond,
            then,
            else_branch,
        })
    }
}

impl Parse for MatchNode {
    fn parse(input: ParseStream) -> Result<Self> {
        let _match: Token![match] = input.parse()?;
        let expr = Expr::parse_without_eager_brace(input)?;

        let content;
        braced!(content in input);
        let mut arms = Vec::new();
        while !content.is_empty() {
            arms.push(content.parse()?);
        }

        Ok(Self { expr, arms })
    }
}

impl Parse for MatchArm {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut pat = syn::Pat::parse_multi_with_leading_vert(input)?;
        // For some reason, syn::Pat does not expose a way to parse the guard so we need to parse it
        // here manually.
        if input.peek(Token![if]) {
            let if_token: Token![if] = input.parse()?;
            let guard = input.parse()?;
            pat = syn::Pat::Guard(syn::PatGuard {
                attrs: Vec::new(),
                pat: Box::new(pat),
                if_token,
                guard: Box::new(guard),
            });
        }

        let _arrow: Token![=>] = input.parse()?;
        // Check if the body is a single expression or a block of code.
        // If it's a block, we parse it as a Root node. If it's a single expression, we parse it as
        // a Node and wrap it in a Root.
        if input.peek(Brace) {
            let body_content;
            braced!(body_content in input);
            let body = body_content.parse()?;
            // If there is a trailing comma after the body, we consume it.
            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            }
            Ok(Self { pat, body })
        } else {
            let body_node: Node = input.parse()?;
            // If there are more tokens after the body, we expect a comma.
            // We also allow a trailing comma after the last arm.
            if !input.is_empty() {
                let _comma: Token![,] = input.parse()?;
            }
            Ok(Self {
                pat,
                body: Root(vec![body_node]),
            })
        }
    }
}

#[cfg(test)]
mod slot_tests {
    use quote::quote;

    use super::*;

    #[test]
    fn rejects_duplicate_and_conflicting_slots() {
        for (input, expected) in [
            (
                quote! { Widget { :footer {} :r#footer {} } },
                "duplicate named slot",
            ),
            (
                quote! { Widget { :prop:footer {} :prop:footer {} } },
                "duplicate named slot",
            ),
            (
                quote! { Widget(footer = value) { :footer {} } },
                "named slot conflicts with an attribute on this tag",
            ),
            (
                quote! { Widget(footer(value)) { :footer {} } },
                "named slot conflicts with an attribute on this tag",
            ),
            (
                quote! { Widget(prop:footer = value) { :prop:footer {} } },
                "named slot conflicts with an attribute on this tag",
            ),
            (
                quote! { Widget { :children {} "body" } },
                "the `children` slot conflicts with unnamed children",
            ),
        ] {
            let Err(err) = syn::parse2::<Root>(input) else {
                panic!("expected a parse error: {expected}");
            };
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn slots_belong_to_the_immediately_enclosing_tag() {
        for input in [
            quote! { :footer {} },
            quote! { Widget { if ready { :footer {} } } },
            quote! { Widget { :outer { :inner {} } } },
        ] {
            let Err(err) = syn::parse2::<Root>(input) else {
                panic!("expected a parse error for misplaced slot");
            };
            assert_eq!(
                err.to_string(),
                "named slots must be direct children of a tag"
            );
        }
        assert!(
            syn::parse2::<Root>(quote! {
                Widget { :outer { Nested { :inner {} } } }
            })
            .is_ok()
        );
    }
}
