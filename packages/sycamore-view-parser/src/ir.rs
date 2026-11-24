//! Intermediate representation for `view!` macro syntax.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Expr, Ident, LitStr, Path};

/// A list of nodes. This is the top-level syntax node and entry-point for parsing.
pub struct Root(pub Vec<Node>);

pub enum Node {
    /// Represents an element or a component.
    Tag(TagNode),
    /// Represents a raw text node.
    /// For HTML, this is just a text node. For other backends, this may not be allowed by itself
    /// and must be wrapped in some kind of `Label` component.
    Text(TextNode),
    /// A dynamic node whose value is interpolated from an expression.
    Dyn(DynNode),
    /// An `if` node that conditionally renders its children.
    If(IfNode),
    /// A `match` node that conditionally renders its children based on a pattern match.
    Match(MatchNode),
}

/// Mirror of the [`Node`] enum, but without the actual data. This is used for pattern matching on
/// node types.
pub enum NodeType {
    Tag,
    Text,
    Dyn,
    If,
    Match,
}

pub struct TagNode {
    pub ident: TagIdent,
    pub props: Vec<Attribute>,
    pub children: Root,
}

pub enum TagIdent {
    /// A standard Rust path.
    Path(Path),
    /// A hyphenated ident. Can not include any paths.
    /// This is used for custom elements support.
    Hyphenated(String),
}

impl TagIdent {
    pub fn span(&self) -> Span {
        match self {
            Self::Path(path) => path.span(),
            Self::Hyphenated(_) => Span::call_site(),
        }
    }
}

/// Represents an attribute on an element or component.
pub enum Attribute {
    /// Syntax: `<ident>=<value>`.
    Plain { ident: Ident, value: Expr },
    /// Syntax: `<hyphenated-name>=<value>`.
    PlainHyphenated { ident: String, value: Expr },
    /// Syntax: `"<quoted-name>"=<value>`.
    PlainQuoted { ident: String, value: Expr },
    /// Syntax: `<dir>:<ident>=<value>`.
    Directive {
        dir: Ident,
        ident: Ident,
        value: Expr,
    },
    /// Syntax: `..<value>`
    Spread { value: Expr },
}

pub struct TextNode {
    pub value: LitStr,
    /// Format arguments, or `None` for a static text node.
    pub format_args: Option<Vec<Expr>>,
}

pub struct DynNode {
    pub value: Expr,
}

pub struct IfNode {
    pub cond: Expr,
    pub then: Root,
    pub else_branch: Option<Root>,
}

pub struct MatchNode {
    pub expr: Expr,
    pub arms: Vec<MatchArm>,
}

pub struct MatchArm {
    pub pat: syn::Pat,
    pub body: Root,
}
