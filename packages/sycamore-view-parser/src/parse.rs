//! Parse syntax for `view!` macro.

use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::token::{Brace, Paren};
use syn::{Ident, LitStr, Result, Token, braced, parenthesized, token};

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
        } else if input.peek(Token![::]) || input.peek(Ident::peek_any) {
            Some(NodeType::Tag)
        } else {
            None
        }
    }
}

impl Parse for Node {
    fn parse(input: ParseStream) -> Result<Self> {
        let ty = match Self::peek_type(input) {
            Some(ty) => ty,
            None => return Err(input.error("expected a valid node")),
        };

        Ok(match ty {
            NodeType::Tag => Self::Tag(input.parse()?),
            NodeType::Text => Self::Text(input.parse()?),
            NodeType::Dyn => Self::Dyn(input.parse()?),
        })
    }
}

impl Parse for TagNode {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident = input.parse()?;

        let has_paren = input.peek(token::Paren);
        let attrs = if has_paren {
            let content;
            parenthesized!(content in input);
            content
                .parse_terminated(Prop::parse, Token![,])?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };

        if !has_paren && !input.peek(Brace) {
            return Err(input.error("expected either `(` or `{` after element tag"));
        }

        let mut children = Vec::new();
        if input.peek(Brace) {
            let content;
            braced!(content in input);
            while !content.is_empty() {
                children.push(content.parse()?);
            }
        }

        Ok(Self {
            ident,
            props: attrs,
            children: Root(children),
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

impl Parse for Prop {
    fn parse(input: ParseStream) -> Result<Self> {
        let span = input.span();
        let ty = input.parse()?;
        if !matches!(ty, PropType::Spread) {
            let _eqs: Token![=] = input.parse()?;
        }
        let value = input.parse()?;
        Ok(Self { ty, value, span })
    }
}

impl Parse for PropType {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![..]) {
            let _2dot = input.parse::<Token![..]>()?;
            Ok(Self::Spread)
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
                Ok(Self::PlainHyphenated { ident })
            } else {
                let name: Ident = input.call(Ident::parse_any)?;

                if name == "ref" {
                    Ok(Self::Ref)
                } else if input.peek(Token![:]) {
                    let _colon: Token![:] = input.parse()?;
                    let ident = input.call(Ident::parse_any)?;
                    Ok(Self::Directive { dir: name, ident })
                } else {
                    Ok(Self::Plain { ident: name })
                }
            }
        } else if lookahead.peek(LitStr) {
            let name: String = <LitStr as Parse>::parse(input).map(|s| s.value())?;
            Ok(Self::PlainQuoted { ident: name })
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for TextNode {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            value: input.parse()?,
        })
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
