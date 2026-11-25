mod apis;
mod components;
mod pages;

use apis::types::StorySorting;
use sycamore::prelude::*;
use sycamore_router::{HistoryIntegration, Route, Router};

#[derive(Debug, Route, Clone)]
enum AppRoutes {
    #[to("/")]
    Top,
    #[to("/new")]
    New,
    #[to("/best")]
    Best,
    #[to("/show")]
    Show,
    #[to("/user/<username>")]
    User { username: String },
    #[to("/item/<id>")]
    Item { id: i64 },
    #[not_found]
    NotFound,
}

#[component]
async fn Switch(route: Signal<AppRoutes>) -> View {
    view! {
        match route.get_clone() {
            AppRoutes::Top => {
                pages::stories::Stories(sorting=StorySorting::Top)
            },
            AppRoutes::New => {
                pages::stories::Stories(sorting=StorySorting::New)
            },
            AppRoutes::Best => {
                pages::stories::Stories(sorting=StorySorting::Best)
            },
            AppRoutes::Show => {
                pages::stories::Stories(sorting=StorySorting::Show)
            },
            AppRoutes::User { username } => {
                pages::user::User(username=username)
            },
            AppRoutes::Item { id } => {
                pages::item::Item(id=id)
            },
            AppRoutes::NotFound => {
                "404 Page Not Found"
            },
        }
    }
}

#[component]
fn App() -> View {
    view! {
        Router(
            integration=HistoryIntegration::new(),
            view=|route: Signal<AppRoutes>| {
                view! {
                    div(class="min-h-screen bg-slate-50 text-slate-900") {
                        components::header::Header {}
                        tags::main(class="mx-auto max-w-5xl px-4 py-5 sm:px-6 sm:py-8") {
                            Switch(route=route)
                        }
                    }
                }
            },
        )
    }
}

fn main() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    sycamore::render(App);
}
