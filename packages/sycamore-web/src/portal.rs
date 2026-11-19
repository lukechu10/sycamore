use sycamore_macro::component;

use crate::*;

/// Mounts children into another part of the DOM on the client, including after hydration.
/// Nothing is rendered on the server.
#[component]
pub fn Portal(selector: impl AsRef<str>, children: Children) -> View {
    is_ssr! {
        let _ = (selector, children);
    }
    is_not_ssr! {
        let selector = selector.as_ref().to_owned();
        // Wait for the target to be mounted and for the initial hydration walk to finish.
        on_mount(move || {
            let Some(parent) = document().query_selector(&selector).unwrap() else {
                console_warn!(
                    "element matching selector `{selector}` not found, portal will not be rendered"
                );
                return;
            };

            let start = HtmlNode::create_start_marker().raw;
            let end = HtmlNode::create_end_marker().raw;
            parent.append_child(&start).unwrap();
            parent.append_child(&end).unwrap();
            crate::node::insert_view(&parent, &children.call(), Some(&end));

            // Own the entire range, not just the initial nodes: dynamic parts can replace
            // their contents before this scope is disposed.
            on_cleanup(move || {
                for node in utils::get_nodes_between(&start, &end) {
                    parent.remove_child(&node).unwrap();
                }
                parent.remove_child(&start).unwrap();
                parent.remove_child(&end).unwrap();
            });
        });
    }
    View::new()
}
