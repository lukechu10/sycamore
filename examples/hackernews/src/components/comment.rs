use sycamore::prelude::*;

use crate::apis::types::Comment;

#[component]
pub fn CommentView(comment: Comment) -> View {
    let Comment {
        id,
        by,
        text,
        time,
        kids: _,
        sub_comments,
        r#type: _,
    } = comment;

    let by_url = format!("user/{}", by);

    let replies = if sub_comments.is_empty() {
        View::default()
    } else {
        let expanded = create_signal(true);
        let replies_id = format!("replies-{id}");
        let sub_comments_view = View::from(
            sub_comments
                .into_iter()
                .map(|comment| view! {
                    CommentView(comment=comment)
                })
                .collect::<Vec<View>>(),
        );

        view! {
            div(class="mt-2 flex") {
                button(
                    r#type="button",
                    aria-label=move || if expanded.get() { "Hide replies" } else { "Show replies" },
                    aria-expanded=move || if expanded.get() { "true" } else { "false" },
                    aria-controls=replies_id.clone(),
                    class="group flex min-h-6 shrink-0 cursor-pointer items-stretch gap-2 rounded-sm px-2 text-sm text-emerald-800 focus-visible:outline-2 focus-visible:outline-emerald-700",
                    on_click=move |_| expanded.set(!expanded.get()),
                ) {
                    span(
                        aria-hidden="true",
                        class="w-0.5 rounded-full bg-slate-300 group-hover:bg-emerald-700 group-focus-visible:bg-emerald-700",
                    )
                    span(class="self-center group-hover:underline", hidden=move || expanded.get()) {
                        "Show replies"
                    }
                }
                ul(id=replies_id, class="min-w-0 flex-1 list-none", hidden=move || !expanded.get()) {
                    (sub_comments_view)
                }
            }
        }
    };

    view! {
        li(class="py-3") {
            div(class="mb-2 flex flex-wrap gap-x-3 gap-y-1 text-sm text-slate-600") {
                a(href=by_url, class="font-medium text-emerald-800 hover:underline") { (by) }
                span { (time.format("%D %l:%M %p").to_string()) }
            }
            div(
                class="break-words leading-relaxed [&_p]:mt-2 [&_pre]:overflow-x-auto [&_a]:underline",
                dangerously_set_inner_html=text,
            )
            (replies)
        }
    }
}
