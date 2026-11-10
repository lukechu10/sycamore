//! Utility functions. Intended for internal use only.

use crate::*;

/// Get all nodes between `start` and `end`.
///
/// If `end` is before `start`, all nodes after `start` will be returned.
///
/// The range is exclusive so `start` and `end` will not be included.
#[must_use]
pub fn get_nodes_between(start: &web_sys::Node, end: &web_sys::Node) -> Vec<web_sys::Node> {
    let parent = start.parent_node().unwrap();
    debug_assert_eq!(
        parent,
        end.parent_node().unwrap(),
        "parents of `start` and `end` do not match"
    );

    let mut nodes = Vec::new();

    let mut next = start.next_sibling();
    while let Some(current) = next {
        let tmp = current.next_sibling();
        if &current == end {
            break;
        } else {
            nodes.push(current);
        }
        next = tmp;
    }

    nodes
}

/// Wrap all the nodes in a [`View`] in a document fragment.
///
/// This is useful when the view is dynamically changed without being mounted since this will not
/// update the DOM. Wrapping the nodes in a document fragment will allow you to dynamically update
/// the view while it is detached, and then insert it into the DOM later.
///
/// This only works on the client side.
pub fn wrap_in_document_fragment(view: View) -> View {
    let fragment = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_document_fragment();

    for node in view.as_web_sys() {
        fragment.append_child(&node).unwrap();
    }

    View::from_node(HtmlNode::from_web_sys(fragment.into()))
}

/// Unwraps all the nodes from a document fragment into a flat [`View`].
///
/// If the first node of the view is not a document fragment, returns the view.
pub fn unwrap_from_document_fragment(view: View) -> View {
    if view.nodes.len() != 1 {
        return view;
    }
    let node = view.nodes[0].as_web_sys();
    if node.node_type() != web_sys::Node::DOCUMENT_FRAGMENT_NODE {
        return view;
    }

    let fragment = node.unchecked_ref::<web_sys::DocumentFragment>();

    let mut nodes = Vec::new();
    let mut next = fragment.first_child();
    while let Some(current) = next {
        next = current.next_sibling();
        nodes.push(current);
    }

    View::from_nodes(nodes.into_iter().map(HtmlNode::from_web_sys).collect())
}

/// Create a shallow copy of a view by converting the nodes to web-sys and then converting them
/// back.
pub fn clone_nodes_via_web_sys(view: &View) -> View {
    let nodes = view
        .as_web_sys()
        .into_iter()
        .map(HtmlNode::from_web_sys)
        .collect();
    View::from_nodes(nodes)
}
