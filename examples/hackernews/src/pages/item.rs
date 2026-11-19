use sycamore::prelude::*;

use crate::apis::types::StoryPageData;
use crate::components::comment::CommentView;
use crate::components::show_stories::Story;

#[component]
pub async fn Item(id: i64) -> View {
    let data = crate::apis::get_story(id).await;
    match data {
        Ok(story) => {
            let StoryPageData { item, comments } = story;

            let has_comments = !comments.is_empty();
            let comments_view = View::from(
                comments
                    .into_iter()
                    .map(|comment| {
                        view! {
                            CommentView(comment=comment)
                        }
                    })
                    .collect::<Vec<View>>(),
            );
            view! {
                ul(class="mb-5 list-none") { Story(story=item) }
                section(class="rounded-lg border border-slate-200 bg-white px-4 py-3 sm:px-5") {
                    h2(class="font-semibold") { "Discussion" }
                    if has_comments {
                        ul(class="mt-1 list-none divide-y divide-slate-200") { (comments_view) }
                    } else {
                        p(class="mt-2 text-sm text-slate-600") { "No comments yet." }
                    }
                }
            }
        }
        Err(_) => view! {
            "Error fetching item."
        },
    }
}
