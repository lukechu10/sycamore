use std::fmt;

use sycamore_core::Build;

use crate::View;

/// A special property type to allow the component to accept children.
///
/// Add a field called `children` of this type to your properties struct.
///
/// Children are eagerly evaluated and created even if not used in the component. If you want to
/// lazily evaluate children, use [`LazyChildren`] instead.
///
/// Due to Rust type-system limitations, we cannot implement [`Build`] for [`Children`]. So to
/// instantiate the [`Children`] somewhere in a [`View`], use `children.view` to get the inner
/// [`View`].
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn Row(width: i32, children: Children) -> View {
///     view! {
///         div {
///             (children.view)
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
#[derive(Debug, Default)]
pub struct Children {
    pub view: View,
}

impl<T> From<T> for Children
where
    T: Build<View>,
{
    fn from(value: T) -> Self {
        Self {
            view: value.build(),
        }
    }
}

/// A special property type to allow the component to accept children.
///
/// Lazy children are evaluated and created only when they are used in the component. If you want to
/// eagerly evaluate children, use [`Children`] instead.
pub struct LazyChildren {
    f: Box<dyn FnOnce() -> View>,
}

impl fmt::Debug for LazyChildren {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Children").finish()
    }
}

impl<F, T> From<F> for LazyChildren
where
    F: FnOnce() -> T + 'static,
    T: Build<View> + 'static,
{
    fn from(f: F) -> Self {
        Self {
            f: Box::new(move || f().build()),
        }
    }
}

impl Default for LazyChildren {
    fn default() -> Self {
        Self {
            f: Box::new(View::default),
        }
    }
}

impl LazyChildren {
    /// Instantiates the child view.
    pub fn call(self) -> View {
        (self.f)()
    }

    /// Create a new [`Children`] from a closure.
    pub fn new(f: impl FnOnce() -> View + 'static) -> Self {
        Self { f: Box::new(f) }
    }
}
