use sycamore_core::Build;
use wasm_bindgen::intern;

use super::dom_list::mount_list;
use crate::*;

const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

/// View backend for rendering to the browser DOM.
pub struct DomNode {
    pub(crate) raw: web_sys::Node,

    #[cfg(feature = "hydrate")]
    origin: NodeOrigin,
    // Builders can call `.children()` repeatedly on the same parent.
    #[cfg(feature = "hydrate")]
    hydration_cursor: Option<HydrationCursor>,
}

/// Where the DOM node came from.
#[cfg(feature = "hydrate")]
#[derive(Clone, Copy)]
enum NodeOrigin {
    /// Created from scratch.
    Fresh,
    /// Adopted from the server-rendered HTML.
    Adopted,
    /// Adopted, but its children are managed by `set_inner_html` rather than a view.
    AdoptedRawHtml,
    /// A client text node used to match server text, which cannot carry a hydration key.
    PendingHydrationText,
}

impl Build<View> for DomNode {
    fn build(self) -> View {
        View::from_node(self)
    }
}

impl DomNode {
    /// Create a new `DomNode` from a raw web-sys node.
    fn fresh(raw: web_sys::Node) -> Self {
        Self {
            raw,
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Fresh,
            #[cfg(feature = "hydrate")]
            hydration_cursor: None,
        }
    }

    fn adopt(raw: web_sys::Node) -> Self {
        Self {
            #[cfg(feature = "hydrate")]
            origin: NodeOrigin::Adopted,
            ..Self::fresh(raw)
        }
    }

    /// Validate a complete child sequence. Individual `append_view` calls cannot do this
    /// because a later `.children()` call may consume the remaining siblings.
    #[cfg(feature = "hydrate")]
    #[cfg_attr(debug_assertions, track_caller)]
    pub(crate) fn finish_hydrating_children(&self) {
        if let Some(cursor) = &self.hydration_cursor {
            cursor.expect_exhausted();
        } else if matches!(self.origin, NodeOrigin::Adopted) {
            // Without a `.children()` call, the client expects no children.
            HydrationCursor::new(self.raw.first_child(), None).expect_exhausted();
        }
    }

    fn validate_element(existing: &web_sys::Element, tag: &str, namespace: &str) {
        if cfg!(debug_assertions) {
            let existing_tag = existing.tag_name();
            let existing_namespace = existing.namespace_uri();
            // HTML tag names are ASCII case-insensitive; SVG names retain their case.
            let matches_tag = if namespace == HTML_NAMESPACE {
                existing_tag.eq_ignore_ascii_case(tag)
            } else {
                existing_tag == tag
            };
            assert!(
                matches_tag && existing_namespace.as_deref() == Some(namespace),
                "expected element with tag `{tag}` in namespace `{namespace}`, \
                 but found `{existing_tag}` in namespace {existing_namespace:?}"
            );
        }
    }
}

impl ViewHtmlNode for DomNode {
    fn create_element(tag: Cow<'static, str>) -> Self {
        match ConstructionMode::current() {
            ConstructionMode::Create => {
                Self::fresh(document().create_element(intern(&tag)).unwrap().into())
            }
            ConstructionMode::Hydrate(session) => {
                let key = session.next_key();
                let element = session
                    .claim_element(key)
                    .expect("could not find element with hydration key");
                Self::validate_element(&element, &tag, HTML_NAMESPACE);
                Self::adopt(element.into())
            }
        }
    }

    fn create_element_ns(namespace: &'static str, tag: Cow<'static, str>) -> Self {
        match ConstructionMode::current() {
            ConstructionMode::Create => Self::fresh(
                document()
                    .create_element_ns(Some(namespace), intern(&tag))
                    .unwrap()
                    .into(),
            ),
            ConstructionMode::Hydrate(session) => {
                let key = session.next_key();
                let element = session
                    .claim_element(key)
                    .expect("could not find element with hydration key");
                Self::validate_element(&element, &tag, namespace);
                Self::adopt(element.into())
            }
        }
    }

    fn create_text_node(text: Cow<'static, str>) -> Self {
        match ConstructionMode::current() {
            ConstructionMode::Create => Self::fresh(document().create_text_node(&text).into()),
            ConstructionMode::Hydrate(_session) => Self {
                #[cfg(feature = "hydrate")]
                origin: NodeOrigin::PendingHydrationText,
                ..Self::fresh(document().create_text_node(&text).into())
            },
        }
    }

    fn create_start_marker() -> Self {
        Self::fresh(document().create_comment("[").into())
    }

    fn create_end_marker() -> Self {
        Self::fresh(document().create_comment("]").into())
    }

    fn set_attribute(&mut self, name: Cow<'static, str>, value: StringAttribute) {
        // FIXME: use setAttributeNS if SVG
        if let Some(value) = value.as_static() {
            if let Some(value) = value {
                self.raw
                    .unchecked_ref::<web_sys::Element>()
                    .set_attribute(&name, value)
                    .unwrap();
            }
        } else {
            let node = self.raw.clone().unchecked_into::<web_sys::Element>();
            create_effect(move || match value.get_clone() {
                Some(value) => node.set_attribute(&name, &value).unwrap(),
                None => node.remove_attribute(&name).unwrap(),
            });
        }
    }

    fn set_bool_attribute(&mut self, name: Cow<'static, str>, value: BoolAttribute) {
        // FIXME: use setAttributeNS if SVG
        if let Some(value) = value.as_static() {
            if *value {
                self.raw
                    .unchecked_ref::<web_sys::Element>()
                    .set_attribute(&name, "")
                    .unwrap();
            }
        } else {
            let node = self.raw.clone().unchecked_into::<web_sys::Element>();
            create_effect(move || {
                if value.get() {
                    node.set_attribute(&name, "").unwrap();
                } else {
                    node.remove_attribute(&name).unwrap();
                }
            });
        }
    }

    fn set_property(&mut self, name: Cow<'static, str>, value: MaybeDyn<JsValue>) {
        if let Some(value) = value.as_static() {
            assert!(js_sys::Reflect::set(&self.raw, &name.as_ref().into(), value).unwrap_throw())
        } else {
            let node = self.raw.clone().unchecked_into::<web_sys::Element>();
            create_effect(move || {
                assert!(
                    js_sys::Reflect::set(&node, &name.as_ref().into(), &value.get_clone())
                        .unwrap_throw()
                )
            });
        }
    }

    fn set_event_handler(
        &mut self,
        name: Cow<'static, str>,
        handler: impl FnMut(web_sys::Event) + 'static,
    ) {
        let node = self.raw.clone();
        let attach = move || {
            let cb = Closure::wrap(Box::new(handler) as Box<dyn FnMut(_)>);
            node.add_event_listener_with_callback(&name, cb.as_ref().unchecked_ref())
                .unwrap();
            on_cleanup(move || {
                node.remove_event_listener_with_callback(&name, cb.as_ref().unchecked_ref())
                    .unwrap();
                drop(cb);
            });
        };
        #[cfg(all(feature = "hydrate", feature = "suspense"))]
        if let ConstructionMode::Hydrate(session) = ConstructionMode::current()
            && session.is_deferred()
        {
            // An async component can construct nodes before its final await. Don't
            // activate their handlers until the range has been matched in place.
            on_mount(attach);
            return;
        }
        attach();
    }

    fn set_inner_html(&mut self, inner_html: Cow<'static, str>) {
        self.raw
            .unchecked_ref::<web_sys::Element>()
            .set_inner_html(&inner_html);
        #[cfg(feature = "hydrate")]
        if matches!(
            self.origin,
            NodeOrigin::Adopted | NodeOrigin::AdoptedRawHtml
        ) {
            self.origin = NodeOrigin::AdoptedRawHtml;
            self.hydration_cursor = None;
        }
    }

    fn as_web_sys(&self) -> &web_sys::Node {
        &self.raw
    }

    fn from_web_sys(node: web_sys::Node) -> Self {
        Self::fresh(node)
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn append_view(&mut self, view: View) {
        #[cfg(feature = "hydrate")]
        if ConstructionMode::current().is_hydrating() {
            let cursor = self.hydration_cursor.get_or_insert_with(|| {
                let mut first = self.raw.first_child();
                if self.raw.node_type() == web_sys::Node::DOCUMENT_NODE
                    && first
                        .as_ref()
                        .is_some_and(|node| node.node_type() == web_sys::Node::DOCUMENT_TYPE_NODE)
                {
                    first = first.and_then(|node| node.next_sibling());
                }
                let mut cursor = HydrationCursor::new(first, None);
                if self
                    .raw
                    .dyn_ref::<web_sys::Element>()
                    .is_some_and(|element| {
                        element.namespace_uri().as_deref() == Some(HTML_NAMESPACE)
                            && super::is_raw_text_element(&element.local_name())
                    })
                {
                    cursor.raw_text_previous_cr = Some(false);
                }
                cursor
            });
            hydrate_view(&self.raw, cursor, &view);
            return;
        }

        insert_view(&self.raw, &view, None);
    }
}

/// Insert a view into the DOM before the `before` node.
///
/// If `before` is `None`, the view will be appended to the end of the parent node.
///
/// # Panics
/// - Panics if `before` is not a child of `parent`.
/// - Panics if `parent` is a text node.
pub(crate) fn insert_view(parent: &web_sys::Node, view: &View, before: Option<&web_sys::Node>) {
    for part in &view.parts {
        if let ViewPart::Node(node) = part {
            parent.insert_before(node.as_web_sys(), before).unwrap();
        } else {
            insert_part(parent, DomPart::from(part), before);
        }
    }
}

/// Insert an empty anchored range into the live parent.
pub(super) fn insert_range(
    parent: &web_sys::Node,
    before: Option<&web_sys::Node>,
) -> (web_sys::Node, web_sys::Node) {
    let start = DomNode::create_start_marker().raw;
    let end = DomNode::create_end_marker().raw;
    parent.insert_before(&start, before).unwrap();
    parent.insert_before(&end, before).unwrap();
    (start, end)
}

fn insert_part(parent: &web_sys::Node, part: DomPart, before: Option<&web_sys::Node>) {
    match part {
        DomPart::Node { raw, .. } => {
            parent.insert_before(&raw, before).unwrap();
        }
        DomPart::Dynamic(content) => {
            let (start, end) = insert_range(parent, before);
            watch_dynamic_range(parent, start, end, content, None::<fn(&web_sys::Node)>);
        }
        DomPart::List(items) => mount_list(parent, items, before),
        #[cfg(feature = "suspense")]
        DomPart::Suspense(view) => super::dom_suspense::mount_suspense(parent, view, before),
        #[cfg(all(feature = "suspense", feature = "hydrate"))]
        DomPart::Deferred(_) => unreachable!("deferred view must first adopt a server range"),
        DomPart::OpaqueRange => {}
    }
}

/// Keep the anchors stable and replace only their interior. For an adopted range,
/// the first run subscribes and attaches the effects to the existing DOM instead.
/// Nested effects belong to this effect, so they are disposed before it replaces their DOM.
fn watch_dynamic_range(
    parent: &web_sys::Node,
    start: web_sys::Node,
    end: web_sys::Node,
    content: Signal<View>,
    mut adopt: Option<impl FnOnce(&web_sys::Node) + 'static>,
) {
    debug_assert_eq!(end.parent_node().as_ref(), Some(parent));
    create_effect(move || {
        content.track();
        // A retained suspense branch can move into a document fragment while hidden.
        let parent = end.parent_node().expect("missing dynamic range parent");
        if let Some(adopt) = adopt.take() {
            adopt(&parent);
            return;
        }

        let parts = content.with_untracked(snapshot_parts);
        debug_assert_eq!(start.parent_node().as_ref(), Some(&parent));
        debug_assert_eq!(end.parent_node().as_ref(), Some(&parent));
        let mut next = start
            .next_sibling()
            .expect("missing dynamic closing anchor");
        while next != end {
            let following = next.next_sibling().expect("missing dynamic closing anchor");
            parent.remove_child(&next).unwrap();
            next = following;
        }

        for part in parts {
            insert_part(&parent, part, Some(&end));
        }
    });
}

/// The unconsumed children of one parent or dynamic range. `end` is exclusive.
#[cfg(feature = "hydrate")]
struct HydrationCursor {
    next: Option<web_sys::Node>,
    end: Option<web_sys::Node>,
    /// Byte offset into `next` when the HTML parser merged adjacent text parts.
    text_offset: usize,
    /// In raw-text elements, remember if a CR at a part boundary already produced
    /// the newline for a following LF. Regular HTML preserves CR via references.
    raw_text_previous_cr: Option<bool>,
}

#[cfg(feature = "hydrate")]
impl HydrationCursor {
    fn new(next: Option<web_sys::Node>, end: Option<web_sys::Node>) -> Self {
        Self {
            next,
            end,
            text_offset: 0,
            raw_text_previous_cr: None,
        }
    }

    fn current(&self) -> Option<&web_sys::Node> {
        self.next
            .as_ref()
            .filter(|node| Some(*node) != self.end.as_ref())
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn mismatch(&self, expected: &str) -> ! {
        let found = self
            .current()
            .map(|node| (node.node_name(), node.node_value()));
        panic!(
            "hydration mismatch: expected {expected}, found {found:?} at text offset {}",
            self.text_offset,
        );
    }

    #[cfg_attr(debug_assertions, track_caller)]
    fn expect_exhausted(&self) {
        if self.next != self.end || self.text_offset != 0 {
            self.mismatch("the end of the child sequence");
        }
    }

    /// Check DOM identity at the current position. Hydration keys choose the element;
    /// the cursor verifies that it is in the expected sibling order.
    fn consume_node(&mut self, expected: &web_sys::Node) {
        if self.text_offset != 0 || self.current() != Some(expected) {
            self.mismatch(&format!("the existing {} node", expected.node_name()));
        }
        self.next = expected.next_sibling();
        if let Some(previous_cr) = &mut self.raw_text_previous_cr {
            *previous_cr = false;
        }
    }

    /// Match a logical text part without splitting the server DOM text node. The offset
    /// survives across appends, so separate `.children()` calls can share one text node.
    fn consume_text(&mut self, expected: &str) {
        if expected.is_empty() {
            return;
        }

        let normalized;
        let mut expected = expected;
        if let Some(previous_cr) = &mut self.raw_text_previous_cr {
            if *previous_cr {
                expected = expected.strip_prefix('\n').unwrap_or(expected);
            }
            *previous_cr = expected.ends_with('\r');
            if expected.contains('\r') {
                normalized = expected.replace("\r\n", "\n").replace('\r', "\n");
                expected = &normalized;
            }
        }
        if expected.is_empty() {
            return;
        }

        let Some(node) = self
            .current()
            .filter(|node| node.node_type() == web_sys::Node::TEXT_NODE)
        else {
            self.mismatch(&format!("text {expected:?}"));
        };
        let text = node.node_value().unwrap_or_default();
        if !text
            .get(self.text_offset..)
            .is_some_and(|remaining| remaining.starts_with(expected))
        {
            self.mismatch(&format!("text {expected:?}"));
        }

        let offset = self.text_offset + expected.len();
        if offset == text.len() {
            self.next = node.next_sibling();
            self.text_offset = 0;
        } else {
            self.text_offset = offset;
        }
    }

    /// Return the server anchors and a cursor bounded by the closing anchor. Match
    /// nested markers by depth, then advance the outer cursor past the entire range.
    #[cfg_attr(debug_assertions, track_caller)]
    fn enter_range(&mut self, markers: (&str, &str)) -> (web_sys::Node, Self, web_sys::Node) {
        let (open, close) = markers;
        let Some(start) = self
            .current()
            .filter(|node| self.text_offset == 0 && marker_value(node).as_deref() == Some(open))
        else {
            self.mismatch(&format!("an opening anchor (`<!--{open}-->`)"));
        };
        let start = start.clone();
        let first = start.next_sibling();
        let mut next = first.clone();
        let mut depth = 0;
        let mut opaque_depth = 0;
        let (opaque_open, opaque_close) = OPAQUE_RANGE_MARKERS;
        while let Some(node) = next {
            if Some(&node) == self.end.as_ref() {
                break;
            }
            match marker_value(&node).as_deref() {
                // Dynamic markers inside an opaque range do not belong to the walk.
                Some(marker) if open != opaque_open && marker == opaque_open => opaque_depth += 1,
                Some(marker) if opaque_depth > 0 && marker == opaque_close => opaque_depth -= 1,
                _ if opaque_depth > 0 => {}
                Some(marker) if marker == open => depth += 1,
                Some(marker) if marker == close && depth == 0 => {
                    self.next = node.next_sibling();
                    return (start, Self::new(first, Some(node.clone())), node);
                }
                Some(marker) if marker == close => depth -= 1,
                _ => {}
            }
            next = node.next_sibling();
        }
        self.mismatch(&format!("a matching closing anchor (`<!--{close}-->`)"));
    }
}

#[cfg(feature = "hydrate")]
fn marker_value(node: &web_sys::Node) -> Option<String> {
    if node.node_type() == web_sys::Node::COMMENT_NODE {
        node.node_value()
    } else {
        None
    }
}

/// Owned handles the renderer can use after releasing a dynamic view's signal borrow.
enum DomPart {
    Node {
        raw: web_sys::Node,
        #[cfg(feature = "hydrate")]
        origin: NodeOrigin,
    },
    Dynamic(Signal<View>),
    List(ListItems),
    #[cfg(feature = "suspense")]
    Suspense(Rc<SuspenseView>),
    #[cfg(all(feature = "suspense", feature = "hydrate"))]
    Deferred(DeferredView),
    OpaqueRange,
}

impl From<&ViewPart> for DomPart {
    fn from(part: &ViewPart) -> Self {
        match part {
            ViewPart::Node(node) => Self::Node {
                raw: node.raw.clone(),
                #[cfg(feature = "hydrate")]
                origin: node.origin,
            },
            ViewPart::Dynamic(signal) => Self::Dynamic(*signal),
            ViewPart::List(items) => Self::List(*items),
            #[cfg(feature = "suspense")]
            ViewPart::Suspense(view) => Self::Suspense(view.clone()),
            #[cfg(all(feature = "suspense", feature = "hydrate"))]
            ViewPart::Deferred(view) => Self::Deferred(*view),
            ViewPart::OpaqueRange { .. } => Self::OpaqueRange,
        }
    }
}

/// Do not hold a reactive arena borrow when mounting parts: mounting a nested
/// dynamic part or a list installs effects in the same arena.
fn snapshot_parts(view: &View) -> smallvec::SmallVec<[DomPart; 1]> {
    view.parts.iter().map(DomPart::from).collect()
}

/// A matched range waiting for its effects, recorded before any subscriptions are installed.
#[cfg(feature = "hydrate")]
enum HydratedPart {
    Dynamic {
        start: web_sys::Node,
        end: web_sys::Node,
        content: Signal<View>,
        nested: Vec<HydratedPart>,
    },
    List {
        start: web_sys::Node,
        end: web_sys::Node,
        items: ListItems,
        rows: Vec<HydratedRow>,
    },
    #[cfg(feature = "suspense")]
    Suspense {
        range: ItemRange,
        initial: ItemRange,
        nested: Vec<HydratedPart>,
        view: Rc<SuspenseView>,
    },
    #[cfg(feature = "suspense")]
    Deferred {
        first: web_sys::Node,
        end: web_sys::Node,
        namespace: std::num::NonZeroU32,
        view: DeferredView,
    },
}

#[cfg(feature = "hydrate")]
struct HydratedRow {
    item: Rc<ListItem>,
    range: ItemRange,
    nested: Vec<HydratedPart>,
}

/// Hydrate one append in two phases: match existing DOM, then subscribe its dynamic
/// ranges. The caller owns the cursor so a later append can resume where this one stops.
#[cfg(feature = "hydrate")]
#[cfg_attr(debug_assertions, track_caller)]
fn hydrate_view(parent: &web_sys::Node, cursor: &mut HydrationCursor, view: &View) {
    attach_hydrated_parts(parent, match_parts(cursor, snapshot_parts(view)));
}

/// Walk one append's parts in sibling order without moving nodes or installing effects.
/// Each dynamic range and each list row must consume its bounded range completely.
#[cfg(feature = "hydrate")]
#[cfg_attr(debug_assertions, track_caller)]
fn match_parts(
    cursor: &mut HydrationCursor,
    parts: impl IntoIterator<Item = DomPart>,
) -> Vec<HydratedPart> {
    let mut ranges = Vec::new();
    for part in parts {
        match part {
            DomPart::Node {
                raw,
                origin: NodeOrigin::PendingHydrationText,
            } => {
                cursor.consume_text(&raw.node_value().unwrap_or_default());
            }
            DomPart::Node { raw, .. } => cursor.consume_node(&raw),
            DomPart::Dynamic(content) => {
                let (start, mut inner, end) = cursor.enter_range(("[", "]"));
                let parts = content.with_untracked(snapshot_parts);
                let nested = match_parts(&mut inner, parts);
                inner.expect_exhausted();
                ranges.push(HydratedPart::Dynamic {
                    start,
                    end,
                    content,
                    nested,
                });
            }
            DomPart::List(items) => {
                let (start, mut inner, end) = cursor.enter_range(("[", "]"));
                let rows = items
                    .rows
                    .get_clone_untracked()
                    .into_iter()
                    .map(|item| {
                        let (start, mut row_cursor, end) = inner.enter_range(("[", "]"));
                        let nested = match_parts(&mut row_cursor, snapshot_parts(&item.view));
                        row_cursor.expect_exhausted();
                        HydratedRow {
                            item,
                            range: ItemRange { start, end },
                            nested,
                        }
                    })
                    .collect();
                inner.expect_exhausted();
                ranges.push(HydratedPart::List {
                    start,
                    end,
                    items,
                    rows,
                });
            }
            #[cfg(feature = "suspense")]
            DomPart::Suspense(view) => {
                let (start, mut outer, end) = cursor.enter_range(("[", "]"));
                let range = ItemRange { start, end };
                let (start, mut inner, end) = outer.enter_range(("[", "]"));
                let fallback = view
                    .initial_fallback
                    .expect("missing initial suspense branch");
                let branch = view.branch(fallback).take();
                let Some(Branch::Ready(branch)) = branch else {
                    panic!("initial suspense branch was not built for hydration");
                };
                let nested = match_parts(&mut inner, snapshot_parts(&branch));
                inner.expect_exhausted();
                outer.expect_exhausted();
                ranges.push(HydratedPart::Suspense {
                    range,
                    initial: ItemRange { start, end },
                    nested,
                    view,
                });
            }
            #[cfg(feature = "suspense")]
            DomPart::Deferred(view) => {
                let (_, inner, end) = cursor.enter_range(("[", "]"));
                let first = inner
                    .current()
                    .expect("missing async hydration namespace")
                    .clone();
                let namespace = marker_value(&first)
                    .and_then(|marker| marker.strip_prefix("async:")?.parse().ok())
                    .expect("invalid async hydration namespace");
                ranges.push(HydratedPart::Deferred {
                    first,
                    end,
                    namespace,
                    view,
                });
            }
            DomPart::OpaqueRange => {
                let _ = cursor.enter_range(OPAQUE_RANGE_MARKERS);
            }
        }
    }
    ranges
}

#[cfg(feature = "hydrate")]
fn attach_hydrated_parts(parent: &web_sys::Node, parts: Vec<HydratedPart>) {
    for part in parts {
        match part {
            HydratedPart::Dynamic {
                start,
                end,
                content,
                nested,
            } => {
                watch_dynamic_range(
                    parent,
                    start,
                    end,
                    content,
                    Some(move |parent: &web_sys::Node| {
                        attach_hydrated_parts(parent, nested);
                    }),
                );
            }
            HydratedPart::List {
                start,
                end,
                items,
                rows,
            } => {
                let mounted = rows
                    .into_iter()
                    .map(
                        |HydratedRow {
                             item,
                             range,
                             nested,
                         }| {
                            assert!(
                                item.range.set(range).is_ok(),
                                "list row was already mounted"
                            );
                            item.scope.run_in(|| attach_hydrated_parts(parent, nested));
                            item
                        },
                    )
                    .collect();
                super::dom_list::watch_list(parent, start, end, items, mounted);
            }
            #[cfg(feature = "suspense")]
            HydratedPart::Suspense {
                range,
                initial,
                nested,
                view,
            } => {
                attach_hydrated_parts(parent, nested);
                super::dom_suspense::watch_suspense(range, view, Some(initial));
            }
            #[cfg(feature = "suspense")]
            HydratedPart::Deferred {
                first,
                end,
                namespace,
                view,
            } => {
                let session =
                    HydrationSession::new_hydrate_range(first.next_sibling(), &end, namespace);
                view.session.set_silent(Some(session));
                create_effect(move || {
                    let parts = view
                        .content
                        .with(|content| content.as_ref().map(snapshot_parts));
                    if let Some(parts) = parts {
                        let parent = end.parent_node().expect("missing async range parent");
                        let mut cursor =
                            HydrationCursor::new(first.next_sibling(), Some(end.clone()));
                        let nested = match_parts(&mut cursor, parts);
                        cursor.expect_exhausted();
                        attach_hydrated_parts(&parent, nested);
                        session.finish();
                    }
                });
            }
        }
    }
}
