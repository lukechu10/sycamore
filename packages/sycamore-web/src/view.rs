//! This module contains the [`View`] struct which represents a view tree.

use std::fmt;

use smallvec::{SmallVec, smallvec};
use sycamore_core::{Build, BuildFromFn, Children};
use sycamore_reactive::Signal;

use crate::*;

/// Represents a view tree.
#[derive(Debug)]
pub struct View {
    pub(crate) parts: SmallVec<[ViewPart; 1]>,
}

pub(crate) enum ViewPart {
    Node(HtmlNode),
    Dynamic(Signal<View>),
    /// A list whose rows are mounted and reconciled as independent DOM ranges.
    List(ListItems),
    #[cfg(all(feature = "suspense", feature = "hydrate"))]
    Deferred(DeferredView),
    #[cfg(all(feature = "suspense", target_arch = "wasm32", not(sycamore_force_ssr)))]
    Suspense(Rc<SuspenseView>),
    /// A server-rendered range that the hydration cursor skips without adopting its contents.
    OpaqueRange {
        #[cfg(any(not(target_arch = "wasm32"), sycamore_force_ssr))]
        content: Box<View>,
    },
}

impl fmt::Debug for ViewPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewPart::Node(_) => write!(f, "Node"),
            ViewPart::Dynamic(_) => write!(f, "Dynamic"),
            ViewPart::List(_) => write!(f, "List"),
            #[cfg(all(feature = "suspense", feature = "hydrate"))]
            ViewPart::Deferred(_) => write!(f, "Deferred"),
            #[cfg(all(feature = "suspense", target_arch = "wasm32", not(sycamore_force_ssr)))]
            ViewPart::Suspense(_) => write!(f, "Suspense"),
            ViewPart::OpaqueRange { .. } => write!(f, "OpaqueRange"),
        }
    }
}

impl View {
    /// Create a new view from a list of parts.
    pub(crate) fn new_from_parts(parts: SmallVec<[ViewPart; 1]>) -> Self {
        Self { parts }
    }

    /// Create a new blank view.
    pub fn new() -> Self {
        Self::new_from_parts(SmallVec::new())
    }

    /// Create a new view from a value that implements [`Build`].
    pub fn from<U: Build<Self>>(value: U) -> Self {
        value.build()
    }

    /// Create a new view with a single node.
    pub fn from_node(node: HtmlNode) -> Self {
        Self::new_from_parts(smallvec![ViewPart::Node(node)])
    }

    /// Create a new view with multiple nodes.
    pub fn from_nodes(nodes: Vec<HtmlNode>) -> Self {
        Self::new_from_parts(nodes.into_iter().map(ViewPart::Node).collect())
    }

    /// Create a reactive view from a function returning any value that implements [`Build`]
    /// for this view. An alias to [`ViewNode::create_dynamic_view`].
    pub fn from_dynamic<U: Build<Self> + 'static>(mut f: impl FnMut() -> U + 'static) -> Self {
        Self::new_from_parts(smallvec![ViewPart::Dynamic(*create_memo(
            move || f().build()
        ))])
    }

    pub fn as_web_sys(&self) -> Vec<web_sys::Node> {
        fn extend_web_sys_nodes(nodes: &mut Vec<web_sys::Node>, view: &View) {
            for part in &view.parts {
                match part {
                    ViewPart::Node(node) => nodes.push(node.as_web_sys().clone()),
                    ViewPart::Dynamic(signal) => {
                        signal.with(|view| extend_web_sys_nodes(nodes, view))
                    }
                    ViewPart::List(items) => {
                        is_ssr! {
                            for item in items {
                                extend_web_sys_nodes(nodes, item);
                            }
                        }
                        is_not_ssr! {
                            for item in items.rows.get_clone() {
                                extend_web_sys_nodes(nodes, &item.view);
                            }
                        }
                    }
                    #[cfg(all(feature = "suspense", feature = "hydrate"))]
                    ViewPart::Deferred(deferred) => {
                        deferred.content.with(|content| {
                            if let Some(view) = content {
                                extend_web_sys_nodes(nodes, view);
                            }
                        });
                    }
                    #[cfg(all(
                        feature = "suspense",
                        target_arch = "wasm32",
                        not(sycamore_force_ssr)
                    ))]
                    ViewPart::Suspense(_) => {}
                    ViewPart::OpaqueRange { .. } => {}
                }
            }
        }

        let mut nodes = Vec::new();
        extend_web_sys_nodes(&mut nodes, self);
        nodes
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildFromFn for View {
    fn from_fn(f: impl FnMut() -> Self + 'static) -> Self {
        Self::from_dynamic(f)
    }
}

impl Build<View> for View {
    fn build(self) -> Self {
        self
    }
}

impl Build<View> for Vec<View> {
    fn build(self) -> View {
        View::new_from_parts(self.into_iter().flat_map(|v| v.parts).collect())
    }
}

impl Build<View> for Option<View> {
    fn build(self) -> View {
        self.unwrap_or_default()
    }
}

impl Build<View> for Children<View> {
    fn build(self) -> View {
        self.call()
    }
}

impl<U: Clone + Build<View>> Build<View> for Signal<U> {
    fn build(self) -> View {
        View::from_dynamic(move || self.get_clone())
    }
}
impl<U: Clone + Build<View>> Build<View> for MutSignal<U> {
    fn build(self) -> View {
        (*self).build()
    }
}
impl<U: Clone + Into<Self> + Build<View>> Build<View> for MaybeDyn<U> {
    fn build(self) -> View {
        (move || self.get_clone()).build()
    }
}

macro_rules! impl_build {
    ($($ty:ty),*) => {
        $(
            impl Build<View> for $ty {
                fn build(self) -> View {
                    View::from_node(HtmlNode::create_text_node(self.into()))
                }
            }
        )*
    }
}

macro_rules! impl_build_with_to_string {
    ($($ty:ty),*) => {
        $(
            impl Build<View> for $ty {
                fn build(self) -> View {
                    View::from_node(HtmlNode::create_text_node(self.to_string().into()))
                }
            }
        )*
    }
}

impl_build!(&'static str, String, Cow<'static, str>);
impl_build_with_to_string!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);

// Implement `Build<View<U>>` for tuples of types that implement `Build<View<U>>`.
macro_rules! impl_build_from_tuple {
    ($($name:ident),*) => {
        paste::paste! {
            impl<$($name),*> Build<View> for ($($name,)*)
            where
                $($name: Build<View>),*
            {
                fn build(self: ($($name,)*)) -> View {
                    let ($([<$name:lower>]),*) = self;
                    #[allow(unused_mut)]
                    let mut parts = SmallVec::new();
                    $(
                        parts.extend([<$name:lower>].build().parts);
                    )*
                    View { parts }
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
