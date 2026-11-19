use super::*;

#[wasm_bindgen_test]
fn append() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");

        assert_text_content!(p, "12");

        count.update(|count| count.push(3));
        assert_text_content!(p, "123");

        count.update(|count| count.remove(0));
        assert_text_content!(p, "23");
    });
}

#[wasm_bindgen_test]
fn swap_rows() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2, 3]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "123");

        count.update(|count| count.swap(0, 2));
        assert_text_content!(p, "321");

        count.update(|count| count.swap(0, 2));
        assert_text_content!(p, "123");
    });
}

#[wasm_bindgen_test]
fn update_row() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "12");

        count.set(vec![1, 3]);
        assert_text_content!(p, "13");
    });
}

#[wasm_bindgen_test]
fn trigger_with_same_data() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "12");

        count.update(|_| {});
        assert_text_content!(p, "12");
    });
}

#[wasm_bindgen_test]
fn delete_row() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2, 3]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "123");

        count.update(|count| count.remove(1));
        assert_text_content!(p, "13");
    });
}

#[wasm_bindgen_test]
fn delete_row_from_start() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "12");

        count.update(|count| count.remove(0));
        assert_text_content!(p, "2");
    });
}

#[wasm_bindgen_test]
fn delete_row_from_end() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "12");

        count.update(|count| count.truncate(1));
        assert_text_content!(p, "1");
    });
}

#[wasm_bindgen_test]
fn clear() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2, 3]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "123");

        count.set(Vec::new());
        assert_text_content!(p, "");
    });
}

#[wasm_bindgen_test]
fn insert_front() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2, 3]);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "123");

        count.update(|count| count.insert(0, 4));
        assert_text_content!(p, "4123");
    });
}

#[wasm_bindgen_test]
fn nested_reactivity() {
    let _ = create_root(|| {
        let count = create_signal(
            vec![1, 2, 3]
                .into_iter()
                .map(create_signal)
                .collect::<Vec<_>>(),
        );

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item.get()) }
                        },
                        key=|item| item.get(),
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "123");

        count.get_clone()[0].set(4);
        assert_text_content!(p, "423");

        let new = create_signal(5);
        count.update(|count| count.push(new));
        assert_text_content!(p, "4235");
    });
}

#[wasm_bindgen_test]
fn fragment_template() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                div {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            span { "The value is: " }
                            strong { (item) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("div");

        assert_text_content!(
            p,
            "\
    The value is: 1\
    The value is: 2"
        );

        count.update(|count| count.push(3));
        assert_text_content!(
            p,
            "\
    The value is: 1\
    The value is: 2\
    The value is: 3"
        );

        count.update(|count| count.remove(0));
        assert_text_content!(
            p,
            "\
    The value is: 2\
    The value is: 3"
        );
    });
}

#[wasm_bindgen_test]
fn template_top_level() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                Keyed(
                    list=count,
                    view=|item| view! {
                        li { (item) }
                    },
                    key=|item| *item,
                )
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("#test-container");

        assert_text_content!(p, "12");

        count.update(|count| count.push(3));
        assert_text_content!(p, "123");

        count.update(|count| count.remove(0));
        assert_text_content!(p, "23");
    });
}

#[wasm_bindgen_test]
fn template_dyn_top_level() {
    let _ = create_root(|| {
        let count = create_signal(vec![1, 2]);

        let view = move || {
            view! {
                div {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            (item)
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("#test-container");

        assert_text_content!(p, "12");

        count.update(|count| count.push(3));
        assert_text_content!(p, "123");

        count.update(|count| count.remove(0));
        assert_text_content!(p, "23");
    });
}

#[wasm_bindgen_test]
fn template_with_other_nodes_at_same_level() {
    let _ = create_root(|| {
        let vec1 = create_signal(vec![1, 2]);
        let vec2 = create_signal(vec![4, 5]);

        let view = move || {
            view! {
                ul {
                    li { "before" }
                    Keyed(
                        list=vec1,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|x| *x,
                    )
                    Keyed(
                        list=vec2,
                        view=|item| view! {
                            li { (item) }
                        },
                        key=|x| *x,
                    )
                    li { "after" }
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let elem = query("ul");

        assert_text_content!(elem, "before1245after");

        vec1.update(|vec1| vec1.push(3));
        assert_text_content!(elem, "before12345after");

        vec1.set(Vec::new());
        assert_text_content!(elem, "before45after");

        vec1.set(vec![1]);
        assert_text_content!(elem, "before145after");
    });
}

#[wasm_bindgen_test]
fn retains_row_effects_and_cleans_up_removed_rows() {
    use std::cell::Cell;
    use std::rc::Rc;

    let c = test_container();
    let _ = create_root(|| {
        let rows = create_signal(vec![1, 0, 2]);
        let suffix = create_signal("a");
        let show = create_signal(true);
        let disposed = Rc::new(Cell::new(0));
        let render_row = {
            let disposed = Rc::clone(&disposed);
            move |i| {
                let disposed = Rc::clone(&disposed);
                on_cleanup(move || disposed.set(disposed.get() + 1));
                if i == 0 {
                    View::new()
                } else {
                    View::from_dynamic(move || format!("{i}{}", suffix.get()))
                }
            }
        };

        sycamore::render_in_scope(
            move || {
                view! {
                    "before"
                    if show.get() {
                        Keyed(list=rows, view=render_row.clone(), key=|i| *i)
                    }
                    "after"
                }
            },
            &c,
        );
        assert_text_content!(c, "before1a2aafter");

        rows.set(vec![2, 0, 1]);
        suffix.set("b");
        assert_text_content!(c, "before2b1bafter");
        assert_eq!(disposed.get(), 0);

        rows.set(vec![2, 1]);
        assert_text_content!(c, "before2b1bafter");
        assert_eq!(disposed.get(), 1);

        show.set(false);
        suffix.set("c");
        assert_text_content!(c, "beforeafter");
        assert_eq!(disposed.get(), 3);
    });
}

#[wasm_bindgen_test]
fn unmount_disposes_a_list_built_outside_its_mount_scope() {
    use std::cell::Cell;
    use std::rc::Rc;

    let c = test_container();
    let _ = create_root(|| {
        let rows = create_signal(vec![1]);
        let suffix = create_signal("a");
        let show = create_signal(true);
        let created = Rc::new(Cell::new(0));
        let disposed = Rc::new(Cell::new(0));
        let view_row = {
            let created = Rc::clone(&created);
            let disposed = Rc::clone(&disposed);
            move |i| {
                created.set(created.get() + 1);
                let disposed = Rc::clone(&disposed);
                on_cleanup(move || disposed.set(disposed.get() + 1));
                View::from_dynamic(move || format!("{i}{}", suffix.get()))
            }
        };
        let mut list = Some(view! { Keyed(list=rows, view=view_row, key=|i| *i) });
        let view = View::from_dynamic(move || {
            if show.get() {
                list.take().unwrap()
            } else {
                View::new()
            }
        });
        sycamore::render_in_scope(|| view, &c);
        assert_text_content!(c, "1a");

        show.set(false);
        suffix.set("b");
        rows.set(vec![2]);
        assert_text_content!(c, "");
        assert_eq!(created.get(), 1);
        assert_eq!(disposed.get(), 1);
    });
}

#[wasm_bindgen_test]
fn issue_795_keyed() {
    let _ = create_root(|| {
        let count = create_signal(vec!['b', 'c', 'g', 'e', 'f', 'd', 'h']);

        let view = move || {
            view! {
                ul {
                    Keyed(
                        list=count,
                        view=|item| view! {
                            li { (item.to_string()) }
                        },
                        key=|item| *item,
                    )
                }
            }
        };

        sycamore::render_in_scope(view, &test_container());

        let p = query("ul");
        assert_text_content!(p, "bcgefdh");

        count.set(vec!['b', 'x', 'y', 'g', 'f', 'e', 'z', 'd', 'h']);
        assert_text_content!(p, "bxygfezdh");
    });
}
