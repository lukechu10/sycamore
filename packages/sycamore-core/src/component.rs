//! Utilities for components and component properties.

use std::fmt;

use sycamore_reactive::*;

/// Runs the given closure inside a new component scope. In other words, this does the following:
/// * Create a new untracked scope (see [`untrack`]).
/// * Call the closure `f` passed to this function.
#[doc(hidden)]
pub fn component_scope<T>(f: impl FnOnce() -> T) -> T {
    untrack(f)
}

/// A trait that is implemented automatically by the `Props` derive macro.
///
/// This is used when constructing components in the `view!` macro.
///
/// # Example
/// Deriving an implementation and using the builder to construct an instance of the struct:
/// ```
/// # use sycamore::prelude::*;
/// #[derive(Props)]
/// struct ButtonProps {
///     color: String,
///     disabled: bool,
/// }
///
/// let builder = <ButtonProps as Props>::builder();
/// let button_props = builder.color("red".to_string()).disabled(false).build();
/// ```
pub trait Props {
    /// The type of the builder. This allows getting the builder type when the name is unknown (e.g.
    /// in a macro).
    type Builder;
    /// Returns the builder for the type.
    /// The builder should be automatically generated using the `Props` derive macro.
    fn builder() -> Self::Builder;
}

/// Make sure that the `Props` trait is implemented for `()` so that components without props can be
/// thought as accepting props of type `()`.
impl Props for () {
    type Builder = UnitBuilder;
    fn builder() -> Self::Builder {
        UnitBuilder
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct UnitBuilder;

impl UnitBuilder {
    pub fn build(self) {}
}

/// A special property type to allow the component to accept children.
///
/// Add a field called `children` of this type to your properties struct.
///
/// Children are lazily evaluated and are only instantiated when they are inserted.
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn Row(width: i32, children: Children) -> View {
///     view! {
///         div {
///             (children)
///         }
///     }
/// }
///
/// # #[component]
/// # fn App() -> View {
/// // Using `Row` somewhere else in your app:
/// view! {
///     Row(width=10) {
///         p { "This is a child node." }
///     }
/// }
/// # }
/// ```
pub struct Children<V> {
    f: Box<dyn FnOnce() -> V>,
}
impl<V> fmt::Debug for Children<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Children").finish()
    }
}

impl<F, V> From<F> for Children<V>
where
    F: FnOnce() -> V + 'static,
{
    fn from(f: F) -> Self {
        Self { f: Box::new(f) }
    }
}

impl<V: Default + 'static> Default for Children<V> {
    fn default() -> Self {
        Self {
            f: Box::new(V::default),
        }
    }
}

impl<V> Children<V> {
    /// Instantiates the child view.
    pub fn call(self) -> V {
        (self.f)()
    }

    /// Create a new [`Children`] from a closure.
    pub fn new(f: impl FnOnce() -> V + 'static) -> Self {
        Self { f: Box::new(f) }
    }
}
