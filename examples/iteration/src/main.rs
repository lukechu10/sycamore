use sycamore::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cat {
    id: &'static str,
    name: &'static str,
}

#[component]
fn App() -> View {
    let items = create_signal(vec![
        Cat {
            id: "J---aiyznGQ",
            name: "Keyboard Cat",
        },
        Cat {
            id: "z_AbfPXTKms",
            name: "Maru",
        },
        Cat {
            id: "OUtn3pvWmpg",
            name: "Henri The Existential Cat",
        },
    ]);

    view! {
        p { "The famous cats of YouTube" }
        ul {
            Indexed(
                list=items,
                view=|Cat { id, name }| {
                    view! {
                        li { a(href=format!("https://www.youtube.com/watch?v={id}")) { (name) } }
                    }
                },
            )
        }
    }
}

fn main() {
    sycamore::render(App);
}
