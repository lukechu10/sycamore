---
title: Rendering Lists
---

# Rendering Lists

## Static lists

There are multiple ways we can render a list in Sycamore. If all our data is
static, we can simply use Rust's iterators to map from `Vec<Data>` to a
`Vec<View>` which can then be turned into a `View`. For example:

```rust
struct Todo {
    task: String,
}

let todos = vec![
    Todo { task: "Learn Rust".to_string() },
    Todo { task: "Learn Sycamore".to_string() },
];

let todos_view = todos.into_iter().map(|todo| view! {
    li { (todo.task) }
}).collect::<Vec<View>>();

view! {
    ul {
        (todos_view)
    }
}
```

This works great for static data. However, it's not so great for dynamic data.
If we turn `list_view` into a closure that returns a `View`, we would get a
dynamically updated list view, but with a problem.

Every time our reactive data is changed, the closure will recreate _all_ the
views for each individual item. Most of the time, however, we are only updating
a small part of our list, such as changing the `task` field of a single todo, or
adding a new todo item to the end of the list. In these cases, we don't want to
re-render the entire list, just change the part that needs to be updated.

For this reason, Sycamore introduces two utility components called `Keyed` and
`Indexed`.

## Indexed lists

An indexed list will automatically diff the previous list with the new list
value to find out which items have changed, and then update them automatically.

```rust
#[derive(Clone, PartialEq, Eq)]
struct Todo {
    task: String
}

let todos = create_signal(vec![...]);

view! {
    ul {
        Indexed(
            list=todos,
            view=|todo| view! {
                li { (todo.task) }
            },
        )
    }
}
```

However, this still has one final issue. If we re-order the items in the list,
`Indexed` has no way of knowing which item is which from the old list. To solve
this, we can use keyed lists instead.

## Keyed lists

A keyed list diffs the previous list with the new list by using a unique key for
each item. This means that we must associate with each item an unique key. For
instance, we can use a simple incrementing counter, or generate random UUIDs
using the [`uuid`](https://docs.rs/uuid/latest/uuid/) crate.

```rust
#[derive(Clone, PartialEq, Eq)]
struct Todo {
    task: String,
    // An unique id associated with each todo.
    // This must be unique, otherwise unexpected things can happen (but not UB).
    id: u32,
}

let todos = create_signal(vec![...]);

view! {
    ul {
        Keyed(
            list=todos,
            view=|todo| view! {
                li { (todo.task) }
            },
            key=|todo| todo.id,
        )
    }
}
```

## Nested Reactivity

One common pattern is called _nested reactivity_. This basically means putting
signals inside signals. For example, if we are building a todo app, we might
want to allow editing the todo task after the todo has already been created.

We might therefore want to change our `Todo` struct to look like:

```rust
#[derive(Clone, PartialEq, Eq)]
struct Todo {
    task: MutSignal<String>,
    id: u32,
}
```

We can then update the list like so:

```rust
let todos = create_signal(vec![...]);

let new_task = "Cook Dinner".to_string();
// MutSignal::update is similar to MutSignal::set but gives you a &mut.
// This allows us to avoid cloning the entire Vec.
todos.update(|todos| todos.push(Todo {
    task: create_signal(new_task),
    // Generate a unique key by using an incrementing counter or using UUIDs.
    id: get_unique_id(),
}));
```

However, this creates a memory leak when we remove items from our list. The
reason is because `create_signal` allocates a new signal in the current
_reactive scope_, and is not deallocated until the parent reactive scope is
disposed. In this case, we are calling `create_signal` in our main `App`
component so the memory will not be deallocated until the app is closed.

We must therefore manually dispose of the signal once it is removed. This is
easily done by adding a `on_cleanup` function inside of the `Keyed` component.

```rust
view! {
    ul {
        Keyed(
            list=todos,
            view=|todo| {
                // Dispose of the signal when this item is removed.
                on_cleanup(move || todo.task.dispose());
                view! { ... }
            },
            key=|todo| todo.id,
        )
    }
}
```

The `on_cleanup` function can be called in any reactive scope and registers a
callback when the surrounding scope is disposed. In this case, `Keyed` creates a
new reactive scope for each item so calling `on_cleanup` inside the view closure
will register the callback when the item is removed from the list.
