//! Mounting and reconciling lists of independently owned row ranges.

use std::collections::HashSet;
use std::iter::once;

use super::dom_node::{insert_range, insert_view};
use crate::*;

pub(super) fn mount_list(parent: &web_sys::Node, items: ListItems, before: Option<&web_sys::Node>) {
    let (start, end) = insert_range(parent, before);
    watch_list(parent, start, end, items, Vec::new());
}

/// Subscribe to row identity and order only. The mapper owns each row's scope, so
/// moving a row does not dispose or recreate its effects.
pub(super) fn watch_list(
    parent: &web_sys::Node,
    start: web_sys::Node,
    end: web_sys::Node,
    items: ListItems,
    mut mounted: Vec<Rc<ListItem>>,
) {
    // Register on the mount owner, not on the effect that reruns on every reorder.
    on_cleanup(move || items.scope.dispose());
    debug_assert_eq!(end.parent_node().as_ref(), Some(parent));
    create_effect(move || {
        let next = items.rows.get_clone();
        if mounted.len() == next.len() && mounted.iter().zip(&next).all(|(a, b)| Rc::ptr_eq(a, b)) {
            return;
        }

        let parent = end.parent_node().expect("missing list range parent");
        reconcile_rows(&parent, &start, &end, &mounted, &next);
        mounted = next;
    });
}

fn reconcile_rows(
    parent: &web_sys::Node,
    start: &web_sys::Node,
    end: &web_sys::Node,
    mounted: &[Rc<ListItem>],
    next: &[Rc<ListItem>],
) {
    if !mounted.is_empty() {
        let retained: HashSet<_> = next.iter().map(Rc::as_ptr).collect();
        for item in mounted {
            if !retained.contains(&Rc::as_ptr(item)) {
                let range = item.range.get().expect("missing mounted row range");
                for node in row_nodes(range) {
                    parent.remove_child(&node).unwrap();
                }
            }
        }
    }

    let mut before = start.next_sibling().expect("missing list closing anchor");
    for item in next {
        let range = if let Some(range) = item.range.get() {
            if range.start != before {
                for node in row_nodes(range) {
                    parent.insert_before(&node, Some(&before)).unwrap();
                }
            }
            range
        } else {
            item.range.get_or_init(|| {
                let (start, end) = insert_range(parent, Some(&before));
                item.scope
                    .run_in(|| insert_view(parent, &item.view, Some(&end)));
                ItemRange { start, end }
            })
        };
        before = range
            .end
            .next_sibling()
            .expect("missing list closing anchor");
    }
    debug_assert_eq!(&before, end);
}

/// Snapshot the live span before moving it. A dynamic child may have replaced
/// its DOM since the row was first built.
fn row_nodes(range: &ItemRange) -> impl Iterator<Item = web_sys::Node> {
    once(range.start.clone())
        .chain(utils::get_nodes_between(&range.start, &range.end))
        .chain(once(range.end.clone()))
}
