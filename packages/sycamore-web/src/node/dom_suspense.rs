//! Switch between retained Suspense branches without rebuilding their views.

use super::dom_node::{insert_range, insert_view};
use crate::*;

pub(super) fn mount_suspense(
    parent: &web_sys::Node,
    view: Rc<SuspenseView>,
    before: Option<&web_sys::Node>,
) {
    let (start, end) = insert_range(parent, before);
    watch_suspense(ItemRange { start, end }, view, None);
}

pub(super) fn watch_suspense(outer: ItemRange, view: Rc<SuspenseView>, initial: Option<ItemRange>) {
    let owner = use_current_scope();
    let content_owner = view.owner;
    on_cleanup(move || content_owner.dispose());

    let mut active = view.initial_fallback;
    let mut retained: [Option<ItemRange>; 2] = [None, None];
    if let Some(fallback) = active {
        retained[usize::from(fallback)] = initial;
    }
    // Blocking and streaming SSR show the resolved output while the equivalent
    // client work runs. Don't flash the fallback before that initial work finishes.
    let mut awaiting_adoption = active == Some(false);

    create_effect(move || {
        let loading = view.loading.get();
        if awaiting_adoption && loading {
            return;
        }
        awaiting_adoption = false;
        if active == Some(loading) {
            return;
        }

        if let Some(fallback) = active {
            let fragment = document().create_document_fragment();
            move_range(
                retained[usize::from(fallback)].as_ref().unwrap(),
                &fragment,
                None,
            );
        }
        let parent = outer
            .end
            .parent_node()
            .expect("missing suspense range parent");
        let slot = &mut retained[usize::from(loading)];
        if let Some(range) = slot {
            move_range(range, &parent, Some(&outer.end));
        } else {
            // Branch effects belong to the mount scope, not the switching effect
            // (whose children are disposed whenever loading changes).
            let branch = view
                .branch(loading)
                .take()
                .expect("suspense branch already mounted");
            let (start, end) = insert_range(&parent, Some(&outer.end));
            owner.run_in(|| {
                let branch = match branch {
                    Branch::Ready(view) => view,
                    Branch::Lazy(children) => {
                        provide_context_in_new_scope(ConstructionMode::Create, || children.call())
                    }
                };
                insert_view(&parent, &branch, Some(&end));
            });
            *slot = Some(ItemRange { start, end });
        }
        active = Some(loading);
    });
}

fn move_range(range: &ItemRange, parent: &web_sys::Node, before: Option<&web_sys::Node>) {
    let nodes = utils::get_nodes_between(&range.start, &range.end);
    for node in std::iter::once(&range.start)
        .chain(&nodes)
        .chain(std::iter::once(&range.end))
    {
        parent.insert_before(node, before).unwrap();
    }
}
