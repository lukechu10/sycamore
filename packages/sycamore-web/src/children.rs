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

/// A special property type to allow the component to accept children that are only created when
/// the component uses them.
///
/// Unlike [`Children`], which is built as soon as the component is constructed, a `LazyChildren`
/// holds a closure that builds the [`View`] when it is [`call`](LazyChildren::call)ed (or inserted
/// into a view with `(children)`). A component that never uses its children never builds them.
///
/// Because the closure is run at most once, `LazyChildren` is consumed when it is used.
///
/// # Passing children
///
/// Any `FnOnce() -> T` closure where `T: Build<View>` can be converted into `LazyChildren`. In
/// `view!`, write the child as a closure, or pass a zero-argument component by name:
///
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn Row(children: LazyChildren) -> View {
///     view! {
///         div {
///             (children)
///         }
///     }
/// }
///
/// # #[component]
/// # fn Greeting() -> View {
/// #     view! { p { "Hello!" } }
/// # }
/// # #[component]
/// # fn App() -> View {
/// view! {
///     // The closure is only run when `Row` renders its children.
///     Row {
///         (move || view! { p { "This is a child node." } })
///     }
///     // A zero-argument component can also be passed by name.
///     Row {
///         (Greeting)
///     }
/// }
/// # }
/// ```
///
/// # Defaults
///
/// `LazyChildren` defaults to an empty [`View`], so a component can accept children without
/// requiring them. Use [`LazyChildren::new`] to build one directly from a closure that returns a
/// [`View`].
///
/// # Example: conditionally using children
///
/// Children are only built if the component actually inserts them. Here, the `if` is a plain Rust
/// expression evaluated once when the component is created, so `children` is used at most once:
///
/// ```
/// # use sycamore::prelude::*;
/// #[component]
/// fn Wrapper(show: bool, children: LazyChildren) -> View {
///     if show {
///         view! { div { (children) } }
///     } else {
///         View::new()
///     }
/// }
/// ```
///
/// Because the closure runs at most once, do not call a `LazyChildren` inside a reactive
/// closure (such as a reactive `if` or `match` in `view!`) that may run multiple times.
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

    /// Creates a new `LazyChildren` from a closure that returns a [`View`].
    ///
    /// The closure is not run until the children are [`call`](LazyChildren::call)ed. This is the
    /// same as converting a closure with [`From`], except that the closure must already return a
    /// [`View`].
    pub fn new(f: impl FnOnce() -> View + 'static) -> Self {
        Self { f: Box::new(f) }
    }
}
