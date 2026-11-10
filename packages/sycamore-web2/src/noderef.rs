//! References to nodes in views.
//!
//! This allows imperatively accessing the node.
//!
//! You can create a [`NodeRef`] by using [`create_node_ref`].

use std::fmt;

use crate::*;

/// A reference to a [`web_sys::Node`].
/// This allows imperative access to the node.
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn Component() -> View {
///     let div_ref = create_node_ref();
///     view! {
///         div(r#ref=div_ref)
///     }
/// }
/// ```
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct NodeRef(Signal<Option<web_sys::Node>>);

impl NodeRef {
    /// Alias to [`create_node_ref`].
    pub fn new() -> Self {
        create_node_ref()
    }

    /// Gets the raw node stored inside the node ref.
    ///
    /// This attempts to cast the node to the specified type.
    ///
    /// # Example
    /// Node refs are generally meant to be accessed in callbacks or in `on_mount`. Accessing the
    /// node ref directly in the body of the component will panic because the node ref has not yet
    /// been set.
    ///
    /// ```
    /// # use sycamore::prelude::*;
    /// # fn Component() -> View {
    /// let div_ref = create_node_ref();
    /// on_mount(move || {
    ///     let node = div_ref.get();
    /// });
    /// view! {
    ///     div(r#ref=div_ref)
    /// }
    /// # }
    /// ```
    ///
    /// # Panics
    /// Panics if the node ref is not set yet or is the wrong type.
    ///
    /// For a non panicking version, see [`NodeRef::try_get`].
    #[track_caller]
    pub fn get(&self) -> web_sys::Node {
        self.try_get().expect("NodeRef is not set")
    }

    /// Tries to get the raw web_sys node stored inside the node ref. Returns `None` if the node
    /// ref has not yet been set (i.e. the node has not yet been rendered into the DOM).
    pub fn try_get(&self) -> Option<web_sys::Node> {
        self.0.get_clone()
    }

    /// Sets the node ref with the specified node.
    ///
    /// This method should be rarely used. Instead, use the `r#ref=` syntax in the `view!` macro to
    /// set the node.
    ///
    /// # Example
    /// Setting the node using the `r#ref=` syntax:
    /// ```
    /// # use sycamore::prelude::*;
    /// #[component]
    /// fn Component() -> View {
    ///     let div_ref = create_node_ref();
    ///     view! {
    ///         div(r#ref=div_ref) // This assigns the node ref a value.
    ///     }
    /// }
    /// ```
    pub fn set(&self, node: Option<web_sys::Node>) {
        self.0.set(node);
    }
}

impl Default for NodeRef {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NodeRef").field(&self.0.get_clone()).finish()
    }
}

/* Hook implementation */

/// Create a new [`NodeRef`].
///
/// The node ref does not point to anything until it is set, either by assigning it to a node in the
/// view or by explicitly calling [`NodeRef::set`].
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// # fn Component() -> View {
/// let node_ref: NodeRef = create_node_ref();
/// # view! {}
/// # }
/// ```
pub fn create_node_ref() -> NodeRef {
    NodeRef(create_signal(None))
}
