---
title: Creating a Todo app
---

# Creating a Todo app

Now that we have gone through all the basics, let's put everything we learned
together to build a simple Todo app.

## Setting up our state

Let's start by defining what our data looks like.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct Todo {
    task: MutSignal<String>,
    completed: MutSignal<bool>,
    id: u32,
}
```

Initially, we won't have any todos so we can simply add:

```rust
#[component]
fn App() -> View {
    let todos = create_signal(Vec::new());
    ...
}
```

We will need to, as discussed in the previous section, have a way of generating
unique IDs for all our todos. Here, we will do so with a simple incrementing
counter. In our `App` component, create the `get_next_id` function:

```rust
let next_id = create_signal(0);
// `replace(...)` is the same as `set(...)` but returns the previous value.
let get_next_id = move || next_id.replace(next_id.get() + 1);
```

Let's also abstract away from creating and removing new todos:

```rust
let add_todo = move |task| {
    todos.update(|todos| {
        todos.push(Todo {
            task: create_signal(task),
            completed: create_signal(false),
            id: get_next_id(),
        })
    })
};

let remove_todo = move |id| todos.update(|todos| todos.retain(|todo| todo.id != id));
```

## Creating the UI

### Displaying the list

Let's start by creating a component for displaying the contents of our list.

```rust
#[component]
fn TodoItem(todo: Todo) -> View {
    // Dispose of nested signals when the todo is removed.
    on_cleanup(move || {
        todo.task.dispose();
        todo.completed.dispose();
    });

    // We are using inline styles here which is generally not considered best practice.
    // In real app, you would probably use an external CSS file.
    let style = move || {
        if todo.completed.get() {
            "text-decoration: line-through;"
        } else {
            ""
        }
    };

    view! {
        li {
            span(style=style) { (todo.task) }
        }
    }
}

#[component]
fn TodoList(#[prop(into)] todos: MaybeDyn<Vec<Todo>>) -> View {
    view! {
        ul {
            Keyed(
                list=todos,
                view=|todo| view! { TodoItem(todo=todo) },
                key=|todo| todo.id,
            )
        }
    }
}
```

We've decided here to represent the completed status of a todo by putting a
strikethrough the text.

### Creating a new todo input

Let's also create a component representing the input box for creating a new
todo. This component will accept a callback for adding a todo, which will be
passed from our `App` component with the `add_todo` closure.

```rust
#[component]
fn TodoInput<F>(add_todo: F) -> View
where
    F: Fn(String) + 'static,
{
    let input = create_signal(String::new());

    let on_keydown = move |ev: KeyboardEvent| {
        if ev.key() == "Enter" && !input.with(String::is_empty) {
            add_todo(input.get_clone());
            // Reset the input.
            input.set(String::new());
        }
    };

    view! {
        div {
            "New Todo: "
            input(bind_value=input, on_keydown=on_keydown)
        }
    }
}
```

Here, we also introduce a new concept: **data-binding**. The `bind:*` attribute
binds the value of a signal to the value of the input. That means that every
time the input is changed, the signal will automatically be updated. It also
works the other way around: every time the signal is updated, the input box will
be updated as well.

Finally, we create a new event handler this is triggered on every key press.
This event handler checks if the key is "Enter" and if we actually have
something in the task input. If so, it calls the callback to create a new todo
and resets the input.

### Drawing the components

Let's put all these components together by calling them from `App`:

```rust
view! {
    TodoInput(add_todo=add_todo)
    TodoList(todos=todos)
}
```

Run `trunk serve` and open up your browser at <http://localhost:8080> and see it
work for yourself!

## Modifying todos

Right now, we can add new tasks to our todo list, but not much else. Let's fix
that.

### Changing completed state

Let's allow changing the completed state of a todo by simply clicking on the
text. In the `TodoItem` component, add a new event handler `toggle_completed`
which toggles the completed status of the current todo.

```rust
let toggle_completed = move |_| todo.completed.set(!todo.completed.get());
view! {
    li {
        span(style=style, on_click=toggle_completed) { (todo.task) }
    }
}
```

Since we already wired up the style to reflect whether a todo was completed or
not, this should just work.

### Removing a todo

This is slightly more complicated. Let's create a button for every todo that,
when clicked, removes the todo from the list. So our `TodoItem` component will
need to accept a callback that can remove a todo from the list. Change the
`TodoItem` function to have the following signature:

```rust
#[component]
fn TodoItem<F>(todo: Todo, remove_todo: F) -> View
where
    F: Fn(u32) + Copy + 'static,
{ ... }
```

Of course, since `TodoItem` is called from `TodoList`, we also need `TodoList`
to accept a closure as a prop so we will need to modify `TodoList`:

```rust
#[component]
fn TodoList<F>(#[prop(into)] todos: MaybeDyn<Vec<Todo>>, remove_todo: F) -> View
where
    F: Fn(u32) + Copy + 'static,
{
    view! {
        ul {
            Keyed(
                list=todos,
                view=move |todo| view! { TodoItem(todo=todo, remove_todo=remove_todo) },
                key=|todo| todo.id,
            )
        }
    }
}
```

And finally pass the closure in from `App`:

```rust
view! {
    TodoList(todos=todos, remove_todo=remove_todo)
}
```

Now, we can finally create a button for removing our todo inside `TodoItem`:

```rust
let remove_todo = move |_| remove_todo(todo.id);

view! {
    li {
        span(style=style, on_click=toggle_completed) {
            (todo.task)
        }
        button(on_click=remove_todo) { "Remove" }
    }
}
```

That was quite a lot of work, just for removing a few todos. This reveals a more
general problem, how do we pass data deep into the component hierarchy without
threading it through every single component in between (a pattern known as
"prop-drilling")?

One solution is to use the [Context API](/book/guide/contexts) which we will not
elaborate more on here.

### Editing todos

One final feature we'll add is allow modification of already existing todos.

This time, instead of adding an `is_editing` field to our `Todo` struct, we will
just create a signal directly inside `TodoItem`.

```rust
let is_editing = create_signal(false);
let start_editing = move |_| is_editing.set(true);

let on_keydown = move |ev: KeyboardEvent| {
    if ev.key() == "Enter" && !todo.task.with(String::is_empty) {
        is_editing.set(false);
    }
};

view! {
    li {
        span(style=style, on_click=toggle_completed) {
            if is_editing.get() {
                input(bind_value=todo.task, on_keydown=on_keydown)
            } else {
                (todo.task)
            }
        }
        button(on_click=start_editing, disabled=move || is_editing.get()) { "Edit Task" }
        button(on_click=remove_todo) { "Remove" }
    }
}
```

This creates a new signal representing whether we are currently editing our task
or not. If we are editing, we show a input box where we can change the task. And
similarly to `TodoInput` from before, we listen to the `keydown` event to
determine when we are done editing.

## Storing todos in local storage

Right now, if we refresh the page in the browser, all of our todos are lost
forever. We want to persist our todos across page refreshes. To do so, we can
use the browser's
[local storage API](https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage).

However, our state is currently stored inside Rust data types and local storage
can only store strings. We must therefore serialize our state, and deseralize it
on initialization. Start by adding some more dependencies: `serde` and
`serde_json` for serialization and deserialization, and `web-sys` for accessing
the local storage API. We'll also want to enable the `serde` flag on `sycamore`
so that we can easily serialize `MutSignal`s.

```bash
cargo add serde -F derive
cargo add serde_json
cargo add web-sys -F Storage
cargo add sycamore -F serde
```

Then replace the `create_signal` function call with:

```rust
// Initialize application state from localStorage.
let local_storage = window()
    .local_storage()
    .unwrap()
    .expect("user has not enabled localStorage");

let todos: MutSignal<Vec<Todo>> = if let Ok(Some(app_state)) = local_storage.get_item("todos") {
    serde_json::from_str(&app_state).unwrap_or_default()
} else {
    Default::default()
};
```

This will try to deserialize the state from local storage, if it exists, or set
the state to a blank list otherwise.

Finally, we also want to save the state every time it changes:

```rust
// Set up an effect that runs whenever app_state.todos changes to save the todos to
// localStorage.
create_effect(move || {
    todos.with(|todos| {
        // Also track all nested signals.
        for todo in todos {
            todo.task.track();
            todo.completed.track();
        }
        local_storage
            .set_item("todos", &serde_json::to_string(todos).unwrap())
            .unwrap();
    });
});
```

Now, we have a fully functioning todo app!

## Full Code

Here is the complete code listing for the todo app.

```rust
use serde::{Deserialize, Serialize};
use sycamore::prelude::*;
use web_sys::KeyboardEvent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Todo {
    task: MutSignal<String>,
    completed: MutSignal<bool>,
    id: u32,
}

#[component]
fn TodoItem<F>(todo: Todo, remove_todo: F) -> View
where
    F: Fn(u32) + Copy + 'static,
{
    on_cleanup(move || {
        todo.task.dispose();
        todo.completed.dispose();
    });

    // We are using inline styles here which is generally not considered best practice.
    // In real app, you would probably use an external CSS file.
    let style = move || {
        if todo.completed.get() {
            "text-decoration: line-through;"
        } else {
            ""
        }
    };
    let toggle_completed = move |_| todo.completed.set(!todo.completed.get());
    let remove_todo = move |_| remove_todo(todo.id);

    let is_editing = create_signal(false);
    let start_editing = move |_| is_editing.set(true);

    let on_keydown = move |ev: KeyboardEvent| {
        if ev.key() == "Enter" && !todo.task.with(String::is_empty) {
            is_editing.set(false);
        }
    };

    view! {
        li {
            span(style=style, on_click=toggle_completed) {
                if is_editing.get() {
                    input(bind_value=todo.task, on_keydown=on_keydown)
                } else {
                    (todo.task)
                }
            }
            button(on_click=start_editing, disabled=move || is_editing.get()) { "Edit Task" }
            button(on_click=remove_todo) { "Remove" }
        }
    }
}

#[component]
fn TodoList<F>(#[prop(setter(into))] todos: MaybeDyn<Vec<Todo>>, remove_todo: F) -> View
where
    F: Fn(u32) + Copy + 'static,
{
    view! {
        ul {
            Keyed(
                list=todos,
                view=move |todo| view! { TodoItem(todo=todo, remove_todo=remove_todo) },
                key=|todo| todo.id,
            )
        }
    }
}

#[component]
fn TodoInput<F>(add_todo: F) -> View
where
    F: Fn(String) + 'static,
{
    let input = create_signal(String::new());

    let on_keydown = move |ev: KeyboardEvent| {
        if ev.key() == "Enter" && !input.with(String::is_empty) {
            add_todo(input.get_clone());
            input.set(String::new());
        }
    };

    view! {
        div {
            "New Todo: "
            input(bind_value=input, on_keydown=on_keydown)
        }
    }
}

#[component]
fn App() -> View {
    // Initialize application state from localStorage.
    let local_storage = window()
        .local_storage()
        .unwrap()
        .expect("user has not enabled localStorage");

    let todos: MutSignal<Vec<Todo>> = if let Ok(Some(app_state)) = local_storage.get_item("todos") {
        serde_json::from_str(&app_state).unwrap_or_default()
    } else {
        Default::default()
    };

    // Set up an effect that runs whenever app_state.todos changes to save the todos to
    // localStorage.
    create_effect(move || {
        todos.with(|todos| {
            // Also track all nested signals.
            for todo in todos {
                todo.task.track();
                todo.completed.track();
            }
            local_storage
                .set_item("todos", &serde_json::to_string(todos).unwrap())
                .unwrap();
        });
    });

    let next_id = create_signal(0);
    // `replace(...)` is the same as `set(...)` but returns the previous value.
    let get_next_id = move || next_id.replace(next_id.get() + 1);

    let add_todo = move |task| {
        todos.update(|todos| {
            todos.push(Todo {
                task: create_signal(task),
                completed: create_signal(false),
                id: get_next_id(),
            })
        })
    };

    let remove_todo = move |id| todos.update(|todos| todos.retain(|todo| todo.id != id));

    view! {
        TodoInput(add_todo=add_todo)
        TodoList(todos=todos, remove_todo=remove_todo)
    }
}

fn main() {
    sycamore::render(App);
}
```
