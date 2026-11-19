use sycamore::prelude::*;

#[component]
pub fn NavLink(href: &'static str, title: &'static str) -> View {
    view! {
        a(class="rounded-md px-2 py-1 font-medium text-slate-600 hover:bg-emerald-50 hover:text-emerald-800", href=href) {
            (title)
        }
    }
}

#[component]
pub fn Header() -> View {
    view! {
        header(class="border-b border-slate-200 bg-white") {
            nav(class="mx-auto flex max-w-5xl flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 text-sm sm:gap-x-5 sm:px-6") {
                a(
                    href="/",
                    aria-label="Hacker News home",
                    class="rounded-md bg-emerald-700 px-2.5 py-1.5 font-bold text-white hover:bg-emerald-800",
                ) { "HN" }
                div(class="flex items-center gap-1") {
                    NavLink(href="/new", title="Latest")
                    NavLink(href="/best", title="Best")
                    NavLink(href="/show", title="Show")
                }
                a(
                    href="https://sycamore.dev",
                    target="_blank",
                    class="ml-auto font-medium text-emerald-800 hover:underline",
                ) {
                    span(class="hidden font-normal text-slate-600 sm:inline") { "Built with " }
                    "Sycamore"
                }
            }
        }
    }
}
