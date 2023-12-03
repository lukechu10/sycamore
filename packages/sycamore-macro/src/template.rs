//! Convert a UI tree into an HTML string.

use sycamore_view_parser::ir::{Node, Prop, PropType, Root, TagIdent, TagNode};
use syn::{Expr, ExprLit, Lit};

use crate::view::{is_bool_attr, is_component, is_void_element};

pub struct ViewToString {
    current_id: u32,
    next_id: u32,
}

impl Default for ViewToString {
    fn default() -> Self {
        Self {
            current_id: 0,
            next_id: 1,
        }
    }
}

impl ViewToString {
    /// Increments the next element id and sets the current id to the new id.
    fn inc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.current_id = id;
        id
    }
}

impl ViewToString {
    pub fn element(&mut self, node: &TagNode) -> String {
        assert!(
            !is_component(&node.ident),
            "components should not appear in HTML template"
        );
        let tag = tagident_to_string(&node.ident);
        let static_attributes = self.attributes(&node.props);
        if is_void_element(&tag) {
            assert!(node.children.0.is_empty());
            format!("<{tag}{static_attributes}/>")
        } else {
            let children = self.children(&node.children);
            format!("<{tag}{static_attributes}>{children}</{tag}>")
        }
    }

    pub fn attributes(&mut self, attrs: &[Prop]) -> String {
        let mut buf = String::new();
        for attr in attrs {
            if let Some(name) = attr_is_static(attr) {
                if is_bool_attr(&name) {
                    match &attr.value {
                        Expr::Lit(ExprLit {
                            lit: Lit::Bool(value),
                            ..
                        }) => {
                            if value.value {
                                buf.push_str(&format!(" {name}"));
                            }
                        }
                        _ => unreachable!("static bool attribute must be a literal"),
                    }
                } else {
                    // Stringify the literal.
                    match &attr.value {
                        Expr::Lit(ExprLit {
                            lit: Lit::Str(value),
                            ..
                        }) => {
                            let value = html_escape::encode_double_quoted_attribute(&value.value())
                                .to_string();
                            buf.push_str(&format!(" {name}=\"{value}\""));
                        }
                        _ => unreachable!("static non-bool attribute must be a string literal"),
                    };
                }
            } else {
                // Dynamic attribute.
            }
        }
        buf
    }

    /// Generates the children of the tag.
    pub fn children(&mut self, root: &Root) -> String {
        let mut buf = String::new();
        let mut children = root.0.iter().peekable();
        while let Some(node) = children.next() {
            match node {
                Node::Tag(node) => {
                    buf.push_str(&self.element(node));
                }
                Node::Text(node) => {
                    html_escape::encode_text_to_string(node.value.value(), &mut buf);
                }
                Node::Dyn(node) => {
                    if let Some(Node::Dyn(_)) = children.peek() {
                        buf.push_str("<?>"); // Comment to separate dynamic nodes.
                    } else {
                        // Set the next node as the marker for this dynamic node.
                    }
                }
            }
        }
        buf
    }
}

/// Gets the tag string from a [`TagIdent`]. If the tag is not an element, this panics.
pub fn tagident_to_string(ident: &TagIdent) -> String {
    match ident {
        TagIdent::Path(path) => path.get_ident().unwrap().to_string(),
        TagIdent::Hyphenated(ident) => ident.to_string(),
    }
}

/// Returns whether an attribute is static. If it is, returns the name of the attribute.
pub fn attr_is_static(attr: &Prop) -> Option<String> {
    // First check if the value is a literal of the appropriate type.
    let lit = match &attr.value {
        Expr::Lit(ExprLit { lit, .. }) => lit,
        _ => return None,
    };
    // Get the name of the attribute, if it is not a directive, ref,
    // dangerously_set_inner_html, or spread.
    match &attr.ty {
        PropType::Plain { ident } if ident != "dangerously_set_inner_html" => {
            if is_bool_attr(&ident.to_string()) {
                if matches!(lit, Lit::Bool(_)) {
                    Some(ident.to_string())
                } else {
                    None
                }
            } else {
                if matches!(lit, Lit::Str(_)) {
                    Some(ident.to_string())
                } else {
                    None
                }
            }
        }
        PropType::PlainHyphenated { ident } if matches!(lit, Lit::Str(_)) => Some(ident.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    fn expect(input: TagNode, expected: &str) {
        let actual = ViewToString::default().element(&input);
        assert_eq!(actual, expected);
    }

    #[test]
    fn view_to_string() {
        expect(parse_quote! { div {} }, "<div></div>");
        expect(
            parse_quote! { div { "Hello, world!" } },
            "<div>Hello, world!</div>",
        );
        expect(
            parse_quote! { div(class="my-class") },
            r#"<div class="my-class"></div>"#,
        );
        expect(
            parse_quote! { div(class="my-class", data-n="123") },
            r#"<div class="my-class" data-n="123"></div>"#,
        );
        expect(parse_quote! { img {} }, "<img/>");
        expect(
            parse_quote! { button(disabled=true) },
            "<button disabled></button>",
        );
        expect(parse_quote! { button(disabled=false) }, "<button></button>");
    }

    #[test]
    fn view_to_string_escapes_raw_strings() {
        expect(
            parse_quote! { div { "Hello, <b>world!</b>" } },
            "<div>Hello, &lt;b&gt;world!&lt;/b&gt;</div>",
        );
        expect(
            parse_quote! { div(class="\"") },
            r#"<div class="&quot;"></div>"#,
        );
    }
}
