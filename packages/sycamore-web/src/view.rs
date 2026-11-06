//! This module contains the [`View`] struct which represents a view tree.

use std::fmt;

use smallvec::{SmallVec, smallvec};
use sycamore_core::{Build, BuildFromFn, Children};

use crate::*;

/// Represents a view tree.
///
/// Internally, this stores a list of nodes. This is the main type that is returned from
/// components.
pub struct View<T = HtmlNode> {
    /// The nodes in the view tree.
    pub(crate) nodes: SmallVec<[T; 1]>,
}

impl<T> View<T> {
    /// Create a new blank view.
    pub fn new() -> Self {
        Self {
            nodes: SmallVec::new(),
        }
    }

    /// Create a new view from a value that implements [`Build`].
    pub fn from<U: Build<Self>>(value: U) -> Self {
        value.build()
    }

    /// Create a new view with a single node.
    pub fn from_node(node: T) -> Self {
        Self {
            nodes: smallvec![node],
        }
    }

    /// Create a new view with multiple nodes.
    pub fn from_nodes(nodes: Vec<T>) -> Self {
        Self {
            nodes: nodes.into(),
        }
    }

    /// Create a new view from a function that returns a view. An alias to
    /// [`ViewNode::create_dynamic_view`].
    pub fn from_dynamic<U: Into<Self> + 'static>(f: impl FnMut() -> U + 'static) -> Self
    where
        T: ViewNode,
    {
        T::create_dynamic_view(f)
    }

    /// Create a flat list of all the web-sys nodes in the view.
    pub fn as_web_sys(&self) -> Vec<web_sys::Node>
    where
        T: ViewHtmlNode,
    {
        self.nodes
            .iter()
            .map(|node| node.as_web_sys().clone())
            .collect()
    }
}

impl<T> Default for View<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for View<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View").finish()
    }
}

impl<T: ViewNode> BuildFromFn for View<T> {
    fn from_fn(f: impl FnMut() -> Self + 'static) -> Self {
        Self::from_dynamic(f)
    }
}

impl<T> Build<View<T>> for View<T> {
    fn build(self) -> View<T> {
        self
    }
}

impl<T> Build<View<T>> for Vec<View<T>> {
    fn build(self) -> View<T> {
        View {
            nodes: self.into_iter().flat_map(|v| v.nodes).collect(),
        }
    }
}

impl<T> Build<View<T>> for Option<View<T>> {
    fn build(self) -> View<T> {
        self.unwrap_or_default()
    }
}

impl<T> Build<View<T>> for Children<View<T>> {
    fn build(self) -> View<T> {
        self.call()
    }
}

impl<T: ViewNode, U: Clone + Build<View<T>>> Build<View<T>> for ReadSignal<U> {
    fn build(self) -> View<T> {
        (move || self.get_clone()).build()
    }
}
impl<T: ViewNode, U: Clone + Build<View<T>>> Build<View<T>> for Signal<U> {
    fn build(self) -> View<T> {
        (*self).build()
    }
}
impl<T: ViewNode, U: Clone + Into<Self> + Build<View<T>>> Build<View<T>> for MaybeDyn<U> {
    fn build(self) -> View<T> {
        (move || self.get_clone()).build()
    }
}

macro_rules! impl_build {
    ($($ty:ty),*) => {
        $(
            impl<T: ViewHtmlNode> Build<View<T>> for $ty {
                fn build(self) -> View<T> {
                    View::from_node(T::create_text_node(self.into()))
                }
            }
        )*
    }
}

macro_rules! impl_build_with_to_string {
    ($($ty:ty),*) => {
        $(
            impl<T: ViewHtmlNode> Build<View<T>> for $ty {
                fn build(self) -> View<T> {
                    View::from_node(T::create_text_node(self.to_string().into()))
                }
            }
        )*
    }
}

impl_build!(&'static str, String, Cow<'static, str>);
impl_build_with_to_string!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);

// Implement `From` for all tuples of types that implement `Into<View<U>>`.
macro_rules! impl_build_from_tuple {
    ($($name:ident),*) => {
        paste::paste! {
            impl<U, $($name),*> Build<View<U>> for ($($name,)*)
            where
                $($name: Build<View<U>>),*
            {
                fn build(self: ($($name,)*)) -> View<U> {
                    let ($([<$name:lower>]),*) = self;
                    #[allow(unused_mut)]
                    let mut nodes = SmallVec::new();
                    $(
                        nodes.extend([<$name:lower>].build().nodes);
                    )*
                    View { nodes }
                }
            }
        }
    };
}

impl_build_from_tuple!();
impl_build_from_tuple!(A, B);
impl_build_from_tuple!(A, B, C);
impl_build_from_tuple!(A, B, C, D);
impl_build_from_tuple!(A, B, C, D, E);
impl_build_from_tuple!(A, B, C, D, E, F);
impl_build_from_tuple!(A, B, C, D, E, F, G);
impl_build_from_tuple!(A, B, C, D, E, F, G, H);
impl_build_from_tuple!(A, B, C, D, E, F, G, H, I);
impl_build_from_tuple!(A, B, C, D, E, F, G, H, I, J);

/// A trait that should be implemented for anything that represents a node in the view tree (UI
/// tree).
///
/// Examples include `DomNode` and `SsrNode` which are used to render views to the browser DOM and
/// to a string respectively. This trait can be implemented for other types to create custom render
/// backends.
pub trait ViewNode: Build<View<Self>> + Sized + 'static {
    /// Appends a child to the node. Panics if the node is not an element or other node that can
    /// have children (e.g. text node).
    fn append_child(&mut self, child: Self);

    /// Append a view to this node. Since a view is just a list of nodes, this essentially appends
    /// every node in the view to this node.
    fn append_view(&mut self, view: View<Self>) {
        for node in view.nodes {
            self.append_child(node);
        }
    }

    /// Create a dynamic view from a function that returns a view.
    ///
    /// The returned view will no longer be a function and can be treated as a normal view and,
    /// e.g., appended as a child to another node.
    ///
    /// Some render backends may not support dynamic views (e.g. `SsrNode`). In that case, the
    /// default behavior is to simply evaluate the function as a static view.
    fn create_dynamic_view<U: Into<View<Self>> + 'static>(
        mut f: impl FnMut() -> U + 'static,
    ) -> View<Self> {
        f().into()
    }
}
