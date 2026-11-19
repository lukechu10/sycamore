use sycamore::prelude::*;

use crate::apis::types::UserData;
use crate::components::show_stories::ShowStories;

#[component]
pub async fn User(username: String) -> View {
    let data = crate::apis::get_user_page(&username).await;
    match data {
        Ok(user) => {
            let UserData {
                id,
                karma,
                about,
                submitted: _,
                stories,
            } = user;
            view! {
                section(class="mb-5 rounded-lg border border-slate-200 bg-white p-4 sm:px-5") {
                    h1(class="break-words text-xl font-semibold") { "User: " (id) }
                    p(class="mt-1 text-sm text-slate-600") { (karma) " karma" }
                    p(class="mt-3 break-words leading-relaxed empty:hidden") { (about) }
                }
                ShowStories(stories=stories)
            }
        }
        Err(_) => view! {
            "Error fetching user."
        },
    }
}
