//! Lower interpolated text into a standard Rust format string and arguments.

use syn::{Expr, Ident, LitStr, Result};

use crate::ir::TextNode;

pub(crate) fn parse(value: LitStr) -> Result<TextNode> {
    let source = value.value();
    let mut args = Vec::new();
    // Do not shadow user captures in width or precision specifiers.
    let occupied = &source;
    let mut prefix = "__sycamore_format_arg_".to_owned();
    while occupied.contains(&prefix) {
        prefix.push('_');
    }

    let mut output = String::new();
    let mut static_text = String::new();
    let mut rest = source.as_str();
    let mut dynamic = false;
    let mut capture_count = 0;
    while !rest.is_empty() {
        if rest.starts_with("{{") || rest.starts_with("}}") {
            output.push_str(&rest[..2]);
            static_text.push(rest.chars().next().unwrap());
            rest = &rest[2..];
        } else if let Some(after_open) = rest.strip_prefix('{') {
            dynamic = true;
            let (end, expr) = expression_end(after_open, value.span()).ok_or_else(|| {
                syn::Error::new(value.span(), "invalid or unclosed string interpolation")
            })?;
            output.push('{');
            let name = Ident::new(&format!("{prefix}{capture_count}"), value.span());
            capture_count += 1;
            output.push_str(&name.to_string());
            args.push(syn::parse_quote!(#name = #expr));
            rest = &after_open[end..];
            if let Some(spec) = rest.strip_prefix(':') {
                let end = spec
                    .find('}')
                    .ok_or_else(|| syn::Error::new(value.span(), "unclosed format specifier"))?;
                output.push(':');
                output.push_str(&spec[..end]);
                rest = &spec[end..];
            }
            debug_assert!(rest.starts_with('}'));
            output.push('}');
            rest = &rest[1..];
        } else if rest.starts_with('}') {
            return Err(syn::Error::new(
                value.span(),
                "unmatched `}` in text; use `}}` for a literal brace",
            ));
        } else {
            let ch = rest.chars().next().unwrap();
            output.push(ch);
            static_text.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    Ok(TextNode {
        value: LitStr::new(if dynamic { &output } else { &static_text }, value.span()),
        format_args: dynamic.then_some(args),
    })
}

/// Find a delimiter after a complete Rust expression. Parsing candidate prefixes lets syn
/// handle nested blocks, strings, closures, generic arguments, etc. rather than duplicating
/// Rust's expression grammar. A single colon introduces format options; `::` belongs to Rust.
fn expression_end(source: &str, span: proc_macro2::Span) -> Option<(usize, Expr)> {
    for (index, ch) in source.char_indices() {
        if ch != '}' && ch != ':' {
            continue;
        }
        if ch == ':' && (source[..index].ends_with(':') || source[index + 1..].starts_with(':')) {
            continue;
        }
        let prefix = &source[..index];
        // A delimiter inside a line comment must not terminate the expression. Appending
        // an identifier checks that the prefix ends outside a comment or string token.
        let sentinel = format!("{prefix} __sycamore_interpolation_end");
        let Ok(tokens) = sentinel.parse::<proc_macro2::TokenStream>() else {
            continue;
        };
        if !matches!(tokens.into_iter().last(), Some(proc_macro2::TokenTree::Ident(ident))
            if ident == "__sycamore_interpolation_end")
        {
            continue;
        }
        if let Ok(expr) = LitStr::new(prefix, span).parse::<Expr>() {
            return Some((index, expr));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::LitStr;

    use crate::ir::{Node, Root};

    #[test]
    fn plain_and_escaped_text_stay_static() {
        for (source, expected) in [
            ("hello", "hello"),
            ("{{hello}}", "{hello}"),
            ("雪 {{}}", "雪 {}"),
            ("", ""),
        ] {
            let node = super::parse(LitStr::new(source, proc_macro2::Span::call_site())).unwrap();
            assert_eq!(node.value.value(), expected);
            assert!(node.format_args.is_none());
        }
    }

    #[test]
    fn format_specifiers_are_preserved() {
        let root: Root = syn::parse2(quote! {
            "{item.name:*>width$.precision$} {value:#08x} {0:?}"
            span { "next" }
        })
        .unwrap();
        assert_eq!(root.0.len(), 2);
        let Node::Text(node) = &root.0[0] else {
            panic!("expected text")
        };
        assert_eq!(
            node.value.value(),
            "{__sycamore_format_arg_0:*>width$.precision$} {__sycamore_format_arg_1:#08x} {__sycamore_format_arg_2:?}"
        );
        assert_eq!(node.format_args.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn invalid_interpolations_are_errors_not_panics() {
        for source in [
            "{",
            "}",
            "{}",
            "{:?}",
            "{name",
            "{a +}",
            "{value:02",
            "{name}}",
            "{a b}",
            "{{}",
        ] {
            let result = super::parse(LitStr::new(source, proc_macro2::Span::call_site()));
            assert!(result.is_err(), "accepted {source:?}");
        }
    }

    #[test]
    fn trailing_format_arguments_are_rejected() {
        for tokens in [
            quote!("{name}", name),
            quote!("{name}", name = "world"),
            quote!("{name}",),
            quote!("{name}";),
        ] {
            assert!(syn::parse2::<Root>(tokens).is_err());
        }
    }
}
