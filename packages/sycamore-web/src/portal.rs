use sycamore_core::Build;
use sycamore_macro::component;

use crate::*;

/// A portal into a different part of the DOM. Only renders in client side rendering (CSR) mode.
/// Does nothing in SSR mode.
#[component]
pub fn Portal(selector: impl AsRef<str>, children: Children) -> View {
    if is_not_ssr!() {
        let selector = selector.as_ref();
        let Some(parent) = document().query_selector(selector).unwrap() else {
            console_warn!(
                "element matching selector `{selector}` not found, portal will not be rendered"
            );
            return View::default();
        };

        // Do this in an `on_mount` so that we don't try to hydrate the portal contents.
        on_mount(move || {
            let start = HtmlNode::create_marker_node();
            let start_node = start.as_web_sys().clone();
            let end = HtmlNode::create_marker_node();
            let end_node = end.as_web_sys().clone();
            let children: View = (start, children, end).build();

            let nodes = children.as_web_sys();
            for node in &nodes {
                parent.append_child(node).unwrap();
            }

            on_cleanup(move || {
                let nodes = utils::get_nodes_between(&start_node, &end_node);
                for node in nodes {
                    parent.remove_child(&node).unwrap();
                }
                parent.remove_child(&start_node).unwrap();
                parent.remove_child(&end_node).unwrap();
            });
        });
    }
    View::default()
}
