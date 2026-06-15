use criterion::{Criterion, criterion_group, criterion_main};
use sycamore::prelude::*;

pub fn bench(c: &mut Criterion) {
    c.bench_function("ssr_small", |b| {
        b.iter(|| {
            #[component]
            fn App() -> View {
                view! {
                    div(class="my-container") {
                        p { "Hello World!" }
                    }
                }
            }

            let _ssr = sycamore::render_to_string(|| view! { App {} });
        })
    });

    c.bench_function("ssr_medium", |b| {
        b.iter(|| {
            #[component(inline_props)]
            fn ListItem(value: i32) -> View {
                view! {
                    p {
                        span(class="placeholder")
                        i { (value) }
                        button(class="delete") {
                            i(class="delete-icon")
                        }
                    }
                }
            }

            #[component]
            fn App() -> View {
                view! {
                    div(class="my-container") {
                        Indexed(
                            list=(0i32..=10).collect::<Vec<_>>(),
                            view=|x| view! {
                                ListItem(value=x)
                            }
                        )
                    }
                }
            }

            let _ssr = sycamore::render_to_string(App);
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().noise_threshold(0.05 /* noisy CI */);
    targets = bench
}
criterion_main!(benches);
