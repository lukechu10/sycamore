use serde::{Deserialize, Serialize};
use sycamore::prelude::*;
use sycamore::web::wasm_bindgen::prelude::*;
use uuid::Uuid;
use web_sys::{HtmlInputElement, KeyboardEvent};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Todo {
    title: String,
    completed: bool,
    id: Uuid,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Active,
    Completed,
}

impl Filter {
    fn url(self) -> &'static str {
        match self {
            Filter::All => "#",
            Filter::Active => "#/active",
            Filter::Completed => "#/completed",
        }
    }

    fn get_filter_from_hash() -> Self {
        let hash = window().location().hash().unwrap();

        match hash.as_str() {
            "#/active" => Filter::Active,
            "#/completed" => Filter::Completed,
            _ => Filter::All,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AppState {
    pub todos: Signal<Vec<Signal<Todo>>>,
    pub filter: Signal<Filter>,
}

impl AppState {
    fn add_todo(&self, title: String) {
        let new = create_signal(Todo {
            title,
            completed: false,
            id: Uuid::new_v4(),
        });
        self.todos.update(|todos| todos.push(new));
    }

    fn remove_todo(&self, id: Uuid) {
        self.todos
            .update(|todos| todos.retain(|todo| todo.with(|todo| todo.id) != id));
    }

    fn todos_left(&self) -> usize {
        self.todos.with(|todos| {
            todos.iter().fold(0, |acc, todo| {
                if todo.with(|todo| todo.completed) {
                    acc
                } else {
                    acc + 1
                }
            })
        })
    }

    fn toggle_complete_all(&self) {
        if self.todos_left() == 0 {
            // make all todos active
            for todo in self.todos.get_clone() {
                if todo.with(|todo| todo.completed) {
                    todo.set(Todo {
                        completed: false,
                        ..todo.get_clone()
                    })
                }
            }
        } else {
            // make all todos completed
            for todo in self.todos.get_clone() {
                if !todo.with(|todo| todo.completed) {
                    todo.set(Todo {
                        completed: true,
                        ..todo.get_clone()
                    })
                }
            }
        }
    }

    fn clear_completed(&self) {
        self.todos
            .update(|todos| todos.retain(|todo| !todo.with(|todo| todo.completed)));
    }
}

const KEY: &str = "todos-sycamore";

fn main() {
    console_error_panic_hook::set_once();
    sycamore::render(App);
}

#[component]
fn App() -> View {
    // Initialize application state from localStorage.
    let local_storage = window()
        .local_storage()
        .unwrap()
        .expect("user has not enabled localStorage");

    let todos = if let Ok(Some(app_state)) = local_storage.get_item(KEY) {
        serde_json::from_str(&app_state).unwrap_or_default()
    } else {
        Default::default()
    };
    let app_state = AppState {
        todos,
        filter: create_signal(Filter::get_filter_from_hash()),
    };
    provide_context(app_state);

    // Set up an effect that runs whenever app_state.todos changes to save the todos to
    // localStorage.
    create_effect(move || {
        app_state.todos.with(|todos| {
            for todo in todos {
                todo.track();
            }
            local_storage
                .set_item(KEY, &serde_json::to_string(todos).unwrap())
                .unwrap();
        });
    });

    let todos_empty = create_selector(move || app_state.todos.with(Vec::is_empty));

    view! {
        div(class="todomvc-wrapper") {
            section(class="todoapp") {
                Header {}
                ((!todos_empty.get()).then(|| view! {
                    List {}
                    Footer {}
                }))
            }
            Copyright {}
        }
    }
}

#[component]
pub fn Copyright() -> View {
    view! {
        footer(class="info") {
            p { "Double click to edit a todo" }
            p {
                "Created by "
                a(href="https://github.com/lukechu10", target="_blank") { "lukechu10" }
            }
            p {
                "Part of "
                a(href="http://todomvc.com") { "TodoMVC" }
            }
        }
    }
}

#[component]
pub fn Header() -> View {
    let app_state = use_context::<AppState>();
    let input_value = create_signal(String::new());

    let handle_keyup = move |event: KeyboardEvent| {
        if event.key() == "Enter" {
            let task = input_value.with(|task| task.trim().to_string());

            if !task.is_empty() {
                app_state.add_todo(task);
                // Reset input field.
                input_value.set("".to_string());
            }
        }
    };

    view! {
        header(class="header") {
            h1 { "todos" }
            input(
                class="new-todo",
                placeholder="What needs to be done?",
                bind_value=input_value,
                on_keyup=handle_keyup,
            )
        }
    }
}

#[component]
pub fn Item(todo: Signal<Todo>) -> View {
    let app_state = use_context::<AppState>();

    let title = move || todo.with(|todo| todo.title.clone());
    let completed = create_selector(move || todo.with(|todo| todo.completed));
    let id = todo.with(|todo| todo.id);

    let is_editing = create_signal(false);
    let input_ref = create_node_ref();
    let input_value = create_signal("".to_string());

    let toggle_completed = move |_| todo.update(|todo| todo.completed = !todo.completed);

    let handle_dblclick = move |_| {
        is_editing.set(true);
        input_ref
            .get()
            .unchecked_into::<HtmlInputElement>()
            .focus()
            .unwrap();
        input_value.set(title());
    };

    let handle_blur = move || {
        is_editing.set(false);

        let value = input_value.with(|value| value.trim().to_string());

        if value.is_empty() {
            app_state.remove_todo(id);
        } else {
            todo.update(|todo| todo.title = value)
        }
    };

    let handle_keyup = move |event: KeyboardEvent| match event.key().as_str() {
        "Enter" => handle_blur(),
        "Escape" => is_editing.set(false),
        _ => {}
    };

    let handle_destroy = move |_| {
        app_state.remove_todo(id);
    };

    // We need a separate signal for checked because clicking the checkbox will detach the binding
    // between the attribute and the view.
    let checked = create_signal(false);
    create_effect(move || {
        // Calling checked.set will also update the `checked` property on the input element.
        checked.set(completed.get())
    });

    let class = move || {
        format!(
            "{} {}",
            if completed.get() { "completed" } else { "" },
            if is_editing.get() { "editing" } else { "" }
        )
    };

    view! {
        li(class=move || class()) {
            div(class="view") {
                input(
                    class="toggle",
                    r#type="checkbox",
                    on_input=toggle_completed,
                    bind_checked=checked,
                )
                label(on_dblclick=handle_dblclick) { (title) }
                button(class="destroy", on_click=handle_destroy)
            }

            if is_editing.get() {
                input(
                    r#ref=input_ref,
                    class="edit",
                    bind_value=input_value,
                    on_blur=move |_| handle_blur(),
                    on_keyup=handle_keyup,
                )
            }
        }
    }
}

#[component]
pub fn List() -> View {
    let app_state = use_context::<AppState>();
    let todos_left = create_selector(move || app_state.todos_left());

    let filtered_todos = create_memo(move || {
        app_state
            .todos
            .get_clone()
            .iter()
            .filter(|todo| match app_state.filter.get() {
                Filter::All => true,
                Filter::Active => !todo.with(|todo| todo.completed),
                Filter::Completed => todo.with(|todo| todo.completed),
            })
            .cloned()
            .collect::<Vec<_>>()
    });

    // We need a separate signal for checked because clicking the checkbox will detach the binding
    // between the attribute and the view.
    let checked = create_signal(false);
    create_effect(move || {
        // Calling checked.set will also update the `checked` property on the input element.
        checked.set(todos_left.get() == 0)
    });

    view! {
        section(class="main") {
            input(
                id="toggle-all",
                class="toggle-all",
                r#type="checkbox",
                readonly=true,
                bind_checked=checked,
                on_input=move |_| app_state.toggle_complete_all(),
            )
            label(r#for="toggle-all")

            ul(class="todo-list") {
                Keyed(
                    list=filtered_todos,
                    view=|todo| view! {
                        Item(todo=todo)
                    },
                    key=|todo| todo.with(|todo| todo.id),
                )
            }
        }
    }
}

#[component]
pub fn TodoFilter(filter: Filter) -> View {
    let app_state = use_context::<AppState>();
    let selected = move || filter == app_state.filter.get();
    let set_filter = move |filter| app_state.filter.set(filter);

    view! {
        li {
            a(
                class=move || if selected() { "selected" } else { "" },
                href=filter.url(),
                on_click=move |_| set_filter(filter),
            ) { (format!("{filter:?}")) }
        }
    }
}

#[component]
pub fn Footer() -> View {
    let app_state = use_context::<AppState>();

    let items_text = move || match app_state.todos_left() {
        1 => "item",
        _ => "items",
    };

    let has_completed_todos =
        create_selector(move || app_state.todos_left() < app_state.todos.with(Vec::len));

    let handle_clear_completed = move |_| app_state.clear_completed();

    view! {
        footer(class="footer") {
            span(class="todo-count") {
                strong { (app_state.todos_left()) }
                span { " " (items_text) " left" }
            }
            ul(class="filters") {
                TodoFilter(filter=Filter::All)
                TodoFilter(filter=Filter::Active)
                TodoFilter(filter=Filter::Completed)
            }

            if has_completed_todos.get() {
                button(class="clear-completed", on_click=handle_clear_completed) { "Clear completed" }
            }
        }
    }
}
