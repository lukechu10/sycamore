//! Iteration utility components.
//!
//! Iteration can be either _"keyed"_ or _"non keyed"_ by using the [`Keyed`] or [`Indexed`] utility
//! components respectively.

use std::hash::Hash;

use sycamore_macro::component;

use crate::*;

/// Keyed iteration.
///
/// Use this instead of directly rendering an array of [`View`]s.
/// Using this will minimize re-renders instead of re-rendering every view node on every
/// state change.
///
/// For non keyed iteration, see [`Indexed`].
///
/// # Example
///
/// ```
/// # use sycamore::prelude::*;
/// #[derive(Clone, PartialEq)]
/// struct AnimalInfo {
///     // The name of the animal.
///     name: &'static str,
///     // An unique id to identify the animal.
///     id: u32,
/// }
///
/// # fn App() -> View {
/// let animals = create_signal(vec![
///     AnimalInfo { name: "Dog", id: 1 },
///     AnimalInfo { name: "Cat", id: 2 },
///     AnimalInfo { name: "Fish", id: 3 },
/// ]);
/// view! {
///     ul {
///         Keyed(
///             list=animals,
///             view=|animal| view! {
///                 li { (animal.name) }
///             },
///             key=|animal| animal.id,
///         )
///     }
/// }
/// # }
/// ```
#[component]
pub fn Keyed<T, K, U, List, F, Key>(
    list: List,
    view: F,
    key: Key,
    #[prop(default)] _phantom: std::marker::PhantomData<(T, K, U)>,
) -> View
where
    T: PartialEq + Clone + 'static,
    K: Hash + Eq + 'static,
    U: Into<View>,
    List: Into<MaybeDyn<Vec<T>>> + 'static,
    F: Fn(T) -> U + 'static,
    Key: Fn(&T) -> K + 'static,
{
    is_ssr! {
        let _ = key;
        list_view(ssr_items(list, view))
    }
    is_not_ssr! {
        list_view(ListItems::new(|| {
            map_keyed(list, move |item| ListItem::new(view(item).into()), key)
        }))
    }
}

/// Non keyed iteration (or keyed by index).
///
/// Use this instead of directly rendering an array of
/// [`View`]s. Using this will minimize re-renders instead of re-rendering every single
/// node on every state change.
///
/// For keyed iteration, see [`Keyed`].
///
/// # Example
/// ```
/// # use sycamore::prelude::*;
/// # fn App() -> View {
/// let fib = create_signal(vec![0, 1, 1, 2, 3, 5, 8]);
/// view! {
///     ul {
///         Indexed(
///             list=fib,
///             view=|x| view! {
///                 li { (x) }
///             },
///         )
///     }
/// }
/// # }
/// ```
#[component]
pub fn Indexed<T, U, List, F>(
    list: List,
    view: F,
    #[prop(default)] _phantom: std::marker::PhantomData<(T, U)>,
) -> View
where
    T: PartialEq + Clone + 'static,
    U: Into<View>,
    List: Into<MaybeDyn<Vec<T>>> + 'static,
    F: Fn(T) -> U + 'static,
{
    is_ssr! {
        list_view(ssr_items(list, view))
    }
    is_not_ssr! {
        list_view(ListItems::new(|| {
            map_indexed(list, move |item| ListItem::new(view(item).into()))
        }))
    }
}

fn list_view(items: ListItems) -> View {
    View::new_from_parts(smallvec::smallvec![ViewPart::List(items)])
}

is_ssr! {
    pub(crate) type ListItems = Vec<View>;

    fn ssr_items<T, U>(list: impl Into<MaybeDyn<Vec<T>>>, view: impl Fn(T) -> U) -> ListItems
    where
        T: Clone + 'static,
        U: Into<View>,
    {
        untrack(|| {
            list.into()
                .evaluate()
                .into_iter()
                .map(|item| view(item).into())
                .collect()
        })
    }
}

is_not_ssr! {
    #[derive(Clone, Copy)]
    pub(crate) struct ListItems {
        pub rows: Signal<Vec<Rc<ListItem>>>,
        pub scope: NodeHandle,
    }

    impl ListItems {
        /// Give the mapper and its rows a dedicated scope so unmounting the view
        /// can dispose them even if the list was built in a longer-lived scope.
        fn new(map: impl FnOnce() -> Signal<Vec<Rc<ListItem>>>) -> Self {
            let mut rows = None;
            let scope = create_child_scope(|| rows = Some(map()));
            Self { rows: rows.unwrap(), scope }
        }
    }

    /// A retained row and the scope in which its view was built. The mapper owns
    /// the scope's lifetime; the renderer attaches its effects there on mount.
    pub(crate) struct ListItem {
        pub view: View,
        pub scope: NodeHandle,
        pub range: std::cell::OnceCell<ItemRange>,
    }

    impl ListItem {
        fn new(view: View) -> Rc<Self> {
            Rc::new(Self {
                view,
                scope: use_current_scope(),
                range: std::cell::OnceCell::new(),
            })
        }
    }

    pub(crate) struct ItemRange {
        pub start: web_sys::Node,
        pub end: web_sys::Node,
    }
}
