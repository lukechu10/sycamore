use sycamore_core::Build;
use wasm_bindgen::intern;

use crate::*;

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
    pub(crate) fn finish_hydrating_children(&self) {
        if let Some(cursor) = &self.hydration_cursor {
            cursor.expect_exhausted();
        }
    }

    fn validate_element(existing: &web_sys::Element, tag: &str) {
        if cfg!(debug_assertions) {
            let existing_tag = existing.tag_name().to_lowercase();
            if existing_tag != tag {
                panic!(
                    "expected element with tag `{tag}`, but found element with tag `{existing_tag}`"
                );
            }
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
                Self::validate_element(&element, &tag);
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
                Self::validate_element(&element, &tag);
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
        let cb = Closure::wrap(Box::new(handler) as Box<dyn FnMut(_)>);
        let node = self.raw.clone();
        node.add_event_listener_with_callback(&name, cb.as_ref().unchecked_ref())
            .unwrap();
        on_cleanup(move || {
            node.remove_event_listener_with_callback(&name, cb.as_ref().unchecked_ref())
                .unwrap();
            drop(cb);
        });
    }

    fn set_inner_html(&mut self, inner_html: Cow<'static, str>) {
        self.raw
            .unchecked_ref::<web_sys::Element>()
            .set_inner_html(&inner_html);
    }

    fn as_web_sys(&self) -> &web_sys::Node {
        &self.raw
    }

    fn from_web_sys(node: web_sys::Node) -> Self {
        Self::fresh(node)
    }

    fn append_view(&mut self, view: View) {
        #[cfg(feature = "hydrate")]
        if ConstructionMode::current().is_hydrating() {
            let cursor = self
                .hydration_cursor
                .get_or_insert_with(|| HydrationCursor::new(self.raw.first_child(), None));
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
fn insert_view(parent: &web_sys::Node, view: &View, before: Option<&web_sys::Node>) {
    for part in &view.parts {
        match part {
            ViewPart::Node(node) => {
                if let Some(before) = before {
                    parent
                        .insert_before(node.as_web_sys(), Some(before))
                        .unwrap();
                } else {
                    parent.append_child(node.as_web_sys()).unwrap();
                }
            }
            ViewPart::Dynamic(signal) => {
                let start = HtmlNode::create_start_marker();
                let end = HtmlNode::create_end_marker();
                if let Some(before) = before {
                    parent
                        .insert_before(start.as_web_sys(), Some(before))
                        .unwrap();
                    parent
                        .insert_before(end.as_web_sys(), Some(before))
                        .unwrap();
                } else {
                    parent.append_child(start.as_web_sys()).unwrap();
                    parent.append_child(end.as_web_sys()).unwrap();
                }

                insert_dynamic_view(parent, start.as_web_sys(), *signal, end.as_web_sys());
            }
        }
    }
}

/// Insert a dynamic view into the DOM. This will create an effect that will update the view
/// whenever the signal changes.
///
/// The view will be inserted into the DOM in between the `before` and `after` nodes.
///
/// # Panics
/// - Panics if the `before` and `after` nodes are not initially siblings in the DOM.
/// - Panics if `before` and `after` are not children of `parent`.
/// - Panics if `parent` is a text node.
fn insert_dynamic_view(
    parent: &web_sys::Node,
    start: &web_sys::Node,
    content: Signal<View>,
    end: &web_sys::Node,
) {
    debug_assert!(
        &start.parent_node().unwrap() == parent,
        "`start` node is not a child of `parent`"
    );
    debug_assert!(
        &end.parent_node().unwrap() == parent,
        "`end` node is not a child of `parent`"
    );
    debug_assert!(
        &start.next_sibling().unwrap() == end,
        "`start` and `end` nodes are not siblings"
    );

    let parent = parent.clone();
    let start = start.clone();
    let end = end.clone();
    create_effect_initial(move || {
        content.with(|view| insert_view(&parent, view, Some(&end)));
        (
            Box::new(move || {
                // Clean up the old nodes before inserting the new ones.
                let mut next = start.next_sibling().unwrap();
                while &next != &end {
                    let tmp = next.next_sibling().unwrap();
                    parent.remove_child(&next).unwrap();
                    next = tmp;
                }

                content.with(|view| insert_view(&parent, view, Some(&end)));
            }),
            (),
        )
    })
}

/// The unconsumed children of one parent or dynamic range. `end` is exclusive.
#[cfg(feature = "hydrate")]
struct HydrationCursor {
    next: Option<web_sys::Node>,
    end: Option<web_sys::Node>,
    /// Byte offset into `next` when the HTML parser merged adjacent text parts.
    text_offset: usize,
}

#[cfg(feature = "hydrate")]
impl HydrationCursor {
    fn new(next: Option<web_sys::Node>, end: Option<web_sys::Node>) -> Self {
        Self {
            next,
            end,
            text_offset: 0,
        }
    }

    fn current(&self) -> Option<&web_sys::Node> {
        self.next
            .as_ref()
            .filter(|node| Some(*node) != self.end.as_ref())
    }

    fn mismatch(&self, expected: &str) -> ! {
        let found = self
            .current()
            .map(|node| (node.node_name(), node.node_value()));
        panic!(
            "hydration mismatch: expected {expected}, found {found:?} at text offset {}",
            self.text_offset,
        );
    }

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
    }

    /// Match a logical text part without splitting the server DOM text node. The offset
    /// survives across appends, so separate `.children()` calls can share one text node.
    fn consume_text(&mut self, expected: &str) {
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
    fn enter_range(&mut self) -> (web_sys::Node, Self, web_sys::Node) {
        let Some(start) = self
            .current()
            .filter(|node| self.text_offset == 0 && marker_value(node).as_deref() == Some("["))
        else {
            self.mismatch("a dynamic opening anchor (`<!--[-->`)");
        };
        let start = start.clone();
        let first = start.next_sibling();
        let mut next = first.clone();
        let mut depth = 0;
        while let Some(node) = next {
            if Some(&node) == self.end.as_ref() {
                break;
            }
            match marker_value(&node).as_deref() {
                Some("[") => depth += 1,
                Some("]") if depth == 0 => {
                    self.next = node.next_sibling();
                    return (start, Self::new(first, Some(node.clone())), node);
                }
                Some("]") => depth -= 1,
                _ => {}
            }
            next = node.next_sibling();
        }
        self.mismatch("a matching dynamic closing anchor (`<!--]-->`)");
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

/// Owned handles we can use after releasing the read borrow on a dynamic view.
#[cfg(feature = "hydrate")]
enum HydrationPart {
    Node(web_sys::Node, NodeOrigin),
    Dynamic(Signal<View>),
}

/// Copy DOM and signal handles, not DOM contents. Traversal and effect installation can
/// then run without holding the reactive arena borrow from reading a `Signal<View>`.
#[cfg(feature = "hydrate")]
fn snapshot_parts(view: &View) -> Vec<HydrationPart> {
    view.parts
        .iter()
        .map(|part| match part {
            ViewPart::Node(node) => HydrationPart::Node(node.raw.clone(), node.origin),
            ViewPart::Dynamic(signal) => HydrationPart::Dynamic(*signal),
        })
        .collect()
}

/// A matched dynamic range waiting for its effect. Nested ranges are recorded in DOM
/// order so they can later be installed under the enclosing range's reactive scope.
#[cfg(feature = "hydrate")]
struct HydratedRange {
    start: web_sys::Node,
    end: web_sys::Node,
    content: Signal<View>,
    nested: Vec<HydratedRange>,
}

/// Hydrate one append in two phases: match existing DOM, then subscribe its dynamic
/// ranges. The caller owns the cursor so a later append can resume where this one stops.
#[cfg(feature = "hydrate")]
fn hydrate_view(parent: &web_sys::Node, cursor: &mut HydrationCursor, view: &View) {
    for range in match_parts(cursor, snapshot_parts(view)) {
        attach_hydrated_range(parent, range);
    }
}

/// Walk one append's parts in sibling order without moving nodes or installing effects.
/// Each dynamic part must consume its bounded range completely.
#[cfg(feature = "hydrate")]
fn match_parts(cursor: &mut HydrationCursor, parts: Vec<HydrationPart>) -> Vec<HydratedRange> {
    let mut ranges = Vec::new();
    for part in parts {
        match part {
            HydrationPart::Node(node, NodeOrigin::PendingHydrationText) => {
                cursor.consume_text(&node.node_value().unwrap_or_default());
            }
            HydrationPart::Node(node, _) => cursor.consume_node(&node),
            HydrationPart::Dynamic(content) => {
                let (start, mut inner, end) = cursor.enter_range();
                let parts = content.with_untracked(snapshot_parts);
                let nested = match_parts(&mut inner, parts);
                inner.expect_exhausted();
                ranges.push(HydratedRange {
                    start,
                    end,
                    content,
                    nested,
                });
            }
        }
    }
    ranges
}

#[cfg(feature = "hydrate")]
fn attach_hydrated_range(parent: &web_sys::Node, range: HydratedRange) {
    watch_hydrated_range(
        parent,
        range.start,
        range.end,
        range.content,
        Some(range.nested),
    );
}

/// Keep the anchors stable while replacing only their interior. When `hydrated` is `Some`,
/// the initial DOM was already matched, so the first run only subscribes and attaches nested
/// effects. Otherwise the anchors were just created during an update and need initial content.
///
/// Nested effects belong to this effect's scope, so they are disposed before this range
/// replaces its content.
#[cfg(feature = "hydrate")]
fn watch_hydrated_range(
    parent: &web_sys::Node,
    start: web_sys::Node,
    end: web_sys::Node,
    content: Signal<View>,
    mut hydrated: Option<Vec<HydratedRange>>,
) {
    let parent = parent.clone();
    create_effect(move || {
        content.track();
        if let Some(nested) = hydrated.take() {
            for range in nested {
                attach_hydrated_range(&parent, range);
            }
            return;
        }

        // Take the snapshot before touching the DOM or installing any new effects.
        let parts = content.with_untracked(snapshot_parts);
        assert!(
            start.parent_node().as_ref() == Some(&parent)
                && end.parent_node().as_ref() == Some(&parent),
            "hydrated range anchors no longer belong to their parent",
        );
        let mut next = start.next_sibling();
        while let Some(node) = next {
            if node == end {
                break;
            }
            next = node.next_sibling();
            parent.remove_child(&node).unwrap();
        }

        for part in parts {
            match part {
                HydrationPart::Node(node, _) => {
                    parent.insert_before(&node, Some(&end)).unwrap();
                }
                HydrationPart::Dynamic(content) => {
                    let start = DomNode::create_start_marker().raw;
                    let close = DomNode::create_end_marker().raw;
                    parent.insert_before(&start, Some(&end)).unwrap();
                    parent.insert_before(&close, Some(&end)).unwrap();
                    watch_hydrated_range(&parent, start, close, content, None);
                }
            }
        }
    });
}
