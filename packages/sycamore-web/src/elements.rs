#![allow(non_snake_case)]

use events::EventHandler;

use crate::*;

/// Create an HTML element with `tag`.
// TODO: deprecate
pub(crate) fn element(tag: &'static str) -> HtmlNode {
    HtmlNode::create_element(tag.into())
}

/// Create a SVG element with `tag`.
pub(crate) fn svg_element(tag: &'static str) -> HtmlNode {
    HtmlNode::create_element_ns("http://www.w3.org/2000/svg", tag.into())
}

/// A struct representing a custom element. This can be created by calling [`custom_element`].
pub struct CustomElement(HtmlNode);

/// Create a new custom element with `tag`.
pub fn custom_element(tag: &'static str) -> CustomElement {
    CustomElement(element(tag))
}

impl From<CustomElement> for View {
    fn from(el: CustomElement) -> Self {
        View::from_node(el.0)
    }
}

impl AsHtmlNode for CustomElement {
    fn as_html_node(&mut self) -> &mut HtmlNode {
        &mut self.0
    }
}

impl GlobalProps for CustomElement {}
impl HtmlGlobalAttributes for CustomElement {}

macro_rules! impl_attribute {
    ($(#[$attr:meta])* $v:vis $ident:ident: $ty:ty) => {
        impl_attribute!($(#[$attr])* $v $ident (stringify!($ident)): $ty);
    };
    ($(#[$attr:meta])* $v:vis $ident:ident ($name:expr): $ty:ty) => {
        $(#[$attr])*
        $v fn $ident(mut self, value: $ty) -> Self {
            self.set_attribute($name, value.into());
            self
        }
    }
}

macro_rules! impl_attributes {
    ($(
        $(#[$attr:meta])*
        $v:vis $ident:ident $(($name:literal))?: $ty:ty,
    )*) => {
        $(
            impl_attribute!($(#[$attr])* $v $ident $(($name))*: $ty);
        )*
    };
}

macro_rules! impl_element {
    (
        $(#[$attr:meta])*
        $name:ident {
            $(
                $(#[$prop_attr:meta])*
                $prop:ident $(($prop_name:literal))?: $ty:ty,
            )*
        }
    ) => {
        impl_element!($(#[$attr])* $name (stringify!($name)) {
            $(
                $(#[$prop_attr])*
                $prop $(($prop_name))*: $ty,
            )*
        });
    };
    (
        $(#[$attr:meta])*
        $name:ident ($tag:expr) {
            $(
                $(#[$prop_attr:meta])*
                $prop:ident $(($prop_name:literal))?: $ty:ty,
            )*
        }
    ) => {
        paste::paste! {
            #[doc = "The `<" $name ">` HTML element. This can be created by calling [`" $name "()`]."]
            pub struct [<Html $name:camel>] (HtmlNode);

            #[doc = "Create a `<" $name ">` element."]
            #[doc = ""]
            $(#[$attr])*
            pub fn $name() -> [<Html $name:camel>] {
                [<Html $name:camel>](element($tag))
            }

            impl From<[<Html $name:camel>]> for View {
                fn from(el: [<Html $name:camel>]) -> Self {
                    View::from_node(el.0)
                }
            }

            impl AsHtmlNode for [<Html $name:camel>] {
                fn as_html_node(&mut self) -> &mut HtmlNode {
                    &mut self.0
                }
            }

            impl GlobalProps for [<Html $name:camel>] {}
            impl HtmlGlobalAttributes for [<Html $name:camel>] {}

            #[doc = "Trait that provides attributes for the `<" $name ">` HTML element."]
            pub trait [<Html $name:camel Attributes>]: SetAttribute + Sized {
                impl_attributes! {
                    $(
                        $(#[$prop_attr])*
                        $prop $(($prop_name))*: $ty,
                    )*
                }
            }

            impl [<Html $name:camel Attributes>] for [<Html $name:camel>] {}
        }
    };
}

macro_rules! impl_elements {
    ($(
        $(#[$attr:meta])*
        $name:ident $(($tag:expr))? {
            $(
                $(#[$prop_attr:meta])*
                $prop:ident $(($prop_name:literal))?: $ty:ty,
            )*
        },
    )*) => {
        $(
            impl_element!($(#[$attr])* $name $(($tag))* {
                $(
                    $(#[$prop_attr])*
                    $prop $(($prop_name))*: $ty,
                )*
            });
        )*
        /// Module that includes all the HTML attribute traits and is intended to be glob exported.
        pub mod html_attributes {
            paste::paste! {
                pub use super::{$([<Html $name:camel Attributes>]),*};
            }
        }
    };
}

macro_rules! impl_svg_element {
    (
        $(#[$attr:meta])*
        $name:ident {
            $(
                $(#[$prop_attr:meta])*
                $prop:ident $(($prop_name:literal))?: $ty:ty,
            )*
        }
    ) => {
        impl_svg_element!($(#[$attr])* $name (stringify!($name)) {
            $(
                $(#[$prop_attr])*
                $prop $(($prop_name))*: $ty,
            )*
        });
    };
    (
        $(#[$attr:meta])*
        $name:ident ($tag:expr) {
            $(
                $(#[$prop_attr:meta])*
                $prop:ident $(($prop_name:literal))?: $ty:ty,
            )*
        }
    ) => {
        paste::paste! {
            #[doc = "The `<" $name ">` SVG element. This can be created by calling [`" $name "()`]."]
            pub struct [<Svg $name:camel>] (HtmlNode);

            #[doc = "Create a `<" $name ">` element."]
            #[doc = ""]
            $(#[$attr])*
            pub fn $name() -> [<Svg $name:camel>] {
                [<Svg $name:camel>](svg_element($tag))
            }

            impl From<[<Svg $name:camel>]> for View {
                fn from(el: [<Svg $name:camel>]) -> Self {
                    View::from_node(el.0)
                }
            }

            impl AsHtmlNode for [<Svg $name:camel>] {
                fn as_html_node(&mut self) -> &mut HtmlNode {
                    &mut self.0
                }
            }

            impl GlobalProps for [<Svg $name:camel>] {}
            impl SvgGlobalAttributes for [<Svg $name:camel>] {}

            #[doc = "Trait that provides attributes for the `<" $name ">` SVG element."]
            pub trait [<Svg $name:camel Attributes>]: SetAttribute + Sized {
                impl_attributes! {
                    $(
                        $(#[$prop_attr])*
                        $prop $(($prop_name))*: $ty,
                    )*
                }
            }

            impl [<Svg $name:camel Attributes>] for [<Svg $name:camel>] {}
        }
    };
}

macro_rules! impl_svg_elements {
    ($(
        $(#[$attr:meta])*
        $name:ident $(($tag:expr))? {
            $(
                $(#[$prop_attr:meta])*
                $prop:ident $(($prop_name:literal))?: $ty:ty,
            )*
        },
    )*) => {
        $(
            impl_svg_element!($(#[$attr])* $name $(($tag))* {
                $(
                    $(#[$prop_attr])*
                    $prop $(($prop_name))*: $ty,
                )*
            });
        )*
        /// Module that includes all the Svg attribute traits and is intended to be glob exported.
        pub mod svg_attributes {
            paste::paste! {
                pub use super::{$([<Svg $name:camel Attributes>]),*};
            }
        }
    };
}

/// Definition of all the HTML and SVG elements.
pub mod tags {
    use super::*;

    impl_elements! {
        /// The `<a>` HTML element (or anchor element), with its `href` attribute, creates a hyperlink to web pages, files, email addresses, locations in the same page, or anything else a URL can address.
        ///
        /// Content within each `<a>` should indicate the link's destination. If the `href` attribute is present, pressing the enter key while focused on the `<a>` element will activate it.
        a {
            download: impl Into<StringAttribute>,
            href: impl Into<StringAttribute>,
            hreflang: impl Into<StringAttribute>,
            target: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
            ping: impl Into<StringAttribute>,
            rel: impl Into<StringAttribute>,
        },
        abbr {},
        address {},
        area {
            alt: impl Into<StringAttribute>,
            coords: impl Into<StringAttribute>,
            download: impl Into<StringAttribute>,
            href: impl Into<StringAttribute>,
            hreflang: impl Into<StringAttribute>,
            media: impl Into<StringAttribute>,
            referrerpolicy: impl Into<StringAttribute>,
            ping: impl Into<StringAttribute>,
            rel: impl Into<StringAttribute>,
            shape: impl Into<StringAttribute>,
            target: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
        },
        article {},
        aside {},
        audio {
            autoplay: impl Into<MaybeDyn<bool>>,
            controls: impl Into<MaybeDyn<bool>>,
            crossorigin: impl Into<StringAttribute>,
            muted: impl Into<MaybeDyn<bool>>,
            preload: impl Into<StringAttribute>,
            src: impl Into<StringAttribute>,
            r#loop("loop"): impl Into<MaybeDyn<bool>>,
        },
        b {},
        base {
            href: impl Into<StringAttribute>,
            target: impl Into<StringAttribute>,
        },
        bdi {},
        bdo {},
        blockquote {
            cite: impl Into<StringAttribute>,
        },
        body {},
        br {},
        /// The `<button>` HTML element represents a clickable button, used to submit forms or anywhere in a document for accessible, standard button functionality.
        ///
        /// By default, HTML buttons are presented in a style resembling the platform the user agent runs on, but you can change buttons’ appearance with CSS.
        button {
            autofocus: impl Into<MaybeDyn<bool>>,
            disabled: impl Into<MaybeDyn<bool>>,
            form: impl Into<StringAttribute>,
            formaction: impl Into<StringAttribute>,
            formenctype: impl Into<StringAttribute>,
            formmethod: impl Into<StringAttribute>,
            formnovalidate: impl Into<MaybeDyn<bool>>,
            formtarget: impl Into<StringAttribute>,
            name: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
            value: impl Into<StringAttribute>,
        },
        canvas {
            height: impl Into<StringAttribute>, // TODO: int value
            width: impl Into<StringAttribute>, // TODO: int value
        },
        caption {},
        cite {},
        code {
            language: impl Into<StringAttribute>,
        },
        col {
            span: impl Into<StringAttribute>, // TODO: int value
        },
        colgroup {
            span: impl Into<StringAttribute>, // TODO: int value
        },
        data {
            value: impl Into<StringAttribute>,
        },
        datalist {},
        dd {},
        del {
            cite: impl Into<StringAttribute>,
            datetime: impl Into<StringAttribute>,
        },
        details {
            open: impl Into<MaybeDyn<bool>>,
        },
        dfn {},
        dialog {},
        /// The `<div>` HTML element is the generic container for flow content. It has no effect on the content or layout until styled in some way using CSS (e.g. styling is directly applied to it, or some kind of layout model like Flexbox is applied to its parent element).
        ///
        /// As a "pure" container, the `<div>` element does not inherently represent anything. Instead, it's used to group content so it can be easily styled using the class or id attributes, marking a section of a document as being written in a different language (using the lang attribute), and so on.
        ///
        /// # Usage notes
        /// The `<div>` element should be used only when no other semantic element (such as `<article>` or `<nav>`) is appropriate.
        div {},
        dl {},
        dt {},
        em {},
        embed {
            height: impl Into<StringAttribute>,
            src: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
            width: impl Into<StringAttribute>, // TODO: int value
        },
        fieldset {},
        figcaption {},
        figure {},
        footer {},
        form {
            acceptcharset: impl Into<StringAttribute>,
            action: impl Into<StringAttribute>,
            autocomplete: impl Into<StringAttribute>,
            enctype: impl Into<StringAttribute>,
            method: impl Into<StringAttribute>,
            name: impl Into<StringAttribute>,
            novalidate: impl Into<MaybeDyn<bool>>,
            target: impl Into<StringAttribute>,
        },
        head {},
        header {},
        hgroup {},
        h1 {},
        h2 {},
        h3 {},
        h4 {},
        h5 {},
        h6 {},
        hr {},
        html {},
        i {},
        iframe {
            allow: impl Into<StringAttribute>,
            allowfullscreen: impl Into<MaybeDyn<bool>>,
            allowpaymentrequest: impl Into<MaybeDyn<bool>>,
            height: impl Into<StringAttribute>,
            loading: impl Into<StringAttribute>,
            name: impl Into<StringAttribute>,
            referrerpolicy: impl Into<StringAttribute>,
            sandbox: impl Into<MaybeDyn<bool>>,
            src: impl Into<StringAttribute>,
            srcdoc: impl Into<StringAttribute>,
            width: impl Into<StringAttribute>,
        },
        img {
            alt: impl Into<StringAttribute>,
            crossorigin: impl Into<StringAttribute>,
            decoding: impl Into<StringAttribute>,
            height: impl Into<StringAttribute>,
            ismap: impl Into<MaybeDyn<bool>>,
            loading: impl Into<StringAttribute>,
            referrerpolicy: impl Into<StringAttribute>,
            sizes: impl Into<StringAttribute>,
            src: impl Into<StringAttribute>,
            srcset: impl Into<StringAttribute>,
            usemap: impl Into<StringAttribute>,
            width: impl Into<StringAttribute>,
        },
        /// The `<input>` HTML element is used to create interactive controls for web-based forms in order to accept data from the user; a wide variety of types of input data and control widgets are available, depending on the device and user agent. The `<input>` element is one of the most powerful and complex in all of HTML due to the sheer number of combinations of input types and attributes.
        input {
            accept: impl Into<StringAttribute>,
            alt: impl Into<StringAttribute>,
            autocomplete: impl Into<StringAttribute>,
            autofocus: impl Into<MaybeDyn<bool>>,
            capture: impl Into<StringAttribute>,
            checked: impl Into<MaybeDyn<bool>>,
            directory: impl Into<StringAttribute>,
            disabled: impl Into<MaybeDyn<bool>>,
            form: impl Into<StringAttribute>,
            formaction: impl Into<StringAttribute>,
            formenctype: impl Into<StringAttribute>,
            formmethod: impl Into<StringAttribute>,
            formnovalidate: impl Into<MaybeDyn<bool>>,
            formtarget: impl Into<StringAttribute>,
            height: impl Into<StringAttribute>, // TODO: int value
            initial_checked: impl Into<MaybeDyn<bool>>,
            initial_value: impl Into<StringAttribute>,
            list: impl Into<StringAttribute>,
            max: impl Into<StringAttribute>,
            maxlength: impl Into<StringAttribute>, // TODO: int value
            min: impl Into<StringAttribute>,
            minlength: impl Into<StringAttribute>, // TODO: int value
            multiple: impl Into<MaybeDyn<bool>>,
            name: impl Into<StringAttribute>,
            pattern: impl Into<StringAttribute>,
            placeholder: impl Into<StringAttribute>,
            readonly: impl Into<MaybeDyn<bool>>,
            required: impl Into<MaybeDyn<bool>>,
            size: impl Into<StringAttribute>, // TODO: int value
            spellcheck: impl Into<MaybeDyn<bool>>,
            src: impl Into<StringAttribute>,
            step: impl Into<StringAttribute>,
            tabindex: impl Into<StringAttribute>, // TODO: int value
            r#type("type"): impl Into<StringAttribute>,
            value: impl Into<StringAttribute>,
            width: impl Into<StringAttribute>, // TODO: int value
        },
        ins {
            cite: impl Into<StringAttribute>,
            datetime: impl Into<StringAttribute>,
        },
        kbd {},
        keygen {},
        /// The `<label>` HTML element represents a caption for an item in a user interface.
        ///
        /// Associating a `<label>` with an `<input>` element offers some major advantages:
        /// * The label text is not only visually associated with its corresponding text input; it is programmatically associated with it too. This means that, for example, a screen reader will read out the label when the user is focused on the form input, making it easier for an assistive technology user to understand what data should be entered.
        /// * When a user clicks or touches/taps a label, the browser passes the focus to its associated input (the resulting event is also raised for the input). That increased hit area for focusing the input provides an advantage to anyone trying to activate it — including those using a touch-screen device.
        ///
        /// To associate the `<label>` with an `<input>` element, you need to give the `<input>` an `id` attribute. The `<label>` then needs a for attribute whose value is the same as the input's `id`.
        ///
        /// Alternatively, you can nest the `<input>` directly inside the `<label>`, in which case the `for` and `id` attributes are not needed because the association is implicit:
        ///
        /// ```html
        /// <label>Do you like peas?
        ///   <input type="checkbox" name="peas">
        /// </label>
        /// ```
        /// The form control that a label is labeling is called the labeled control of the label element. Multiple labels can be associated with the same form control:
        ///
        /// ```html
        /// <label for="username">Enter your username:</label>
        /// <input id="username">
        /// <label for="username">Forgot your username?</label>
        /// ```
        /// Elements that can be associated with a `<label>` element include `<button>`, `<input>` (except for `type="hidden"`), `<meter>`, `<output>`, `<progress>`, `<select>` and `<textarea>`.
        label {
            form: impl Into<StringAttribute>,
            r#for("for"): impl Into<StringAttribute>,
        },
        legend {},
        /// The `<li>` HTML element is used to represent an item in a list. It must be contained in a parent element: an ordered list (`<ol>`), an unordered list (`<ul>`), or a menu (`<menu>`). In menus and unordered lists, list items are usually displayed using bullet points. In ordered lists, they are usually displayed with an ascending counter on the left, such as a number or letter.
        li {
            value: impl Into<StringAttribute>, // TODO: int value
        },
        link {
            r#as("as"): impl Into<StringAttribute>,
            crossorigin: impl Into<StringAttribute>,
            href: impl Into<StringAttribute>,
            hreflang: impl Into<StringAttribute>,
            media: impl Into<StringAttribute>,
            rel: impl Into<StringAttribute>,
            sizes: impl Into<StringAttribute>,
            title: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
            integrity: impl Into<StringAttribute>,
        },
        main {},
        map {
            name: impl Into<StringAttribute>,
        },
        mark {},
        menu {},
        menuitem {},
        meta {
            charset: impl Into<StringAttribute>,
            content: impl Into<StringAttribute>,
            http_equiv("http-equiv"): impl Into<StringAttribute>,
            name: impl Into<StringAttribute>,
        },
        meter {
            value: impl Into<StringAttribute>, // TODO: int value
            min: impl Into<StringAttribute>, // TODO: int value
            max: impl Into<StringAttribute>, // TODO: int value
            low: impl Into<StringAttribute>, // TODO: int value
            high: impl Into<StringAttribute>, // TODO: int value
            optimum: impl Into<StringAttribute>, // TODO: int value
            form: impl Into<StringAttribute>,
        },
        nav {},
        noscript {},
        object {
            data: impl Into<StringAttribute>,
            form: impl Into<StringAttribute>,
            height: impl Into<StringAttribute>, // TODO: int value
            name: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
            typemustmatch: impl Into<MaybeDyn<bool>>,
            usemap: impl Into<StringAttribute>,
            width: impl Into<StringAttribute>,
        },
        /// The `<ol>` HTML element represents an ordered list of items — typically rendered as a numbered list.
        ol {
            reversed: impl Into<MaybeDyn<bool>>,
            start: impl Into<StringAttribute>, // TODO: int value
            r#type("type"): impl Into<StringAttribute>,
        },
        optgroup {
            disabled: impl Into<MaybeDyn<bool>>,
            label: impl Into<StringAttribute>,
        },
        option {
            disabled: impl Into<MaybeDyn<bool>>,
            initial_selected: impl Into<MaybeDyn<bool>>,
            label: impl Into<StringAttribute>,
            selected: impl Into<MaybeDyn<bool>>,
            value: impl Into<StringAttribute>,
        },
        output {
            r#for("for"): impl Into<StringAttribute>,
            form: impl Into<StringAttribute>,
            name: impl Into<StringAttribute>,
        },
        /// The `<p>` HTML element represents a paragraph. Paragraphs are usually represented in visual media as blocks of text separated from adjacent blocks by blank lines and/or first-line indentation, but HTML paragraphs can be any structural grouping of related content, such as images or form fields.
        ///
        /// Paragraphs are block-level elements, and notably will automatically close if another block-level element is parsed before the closing `</p>` tag.
        p {},
        param {
            name: impl Into<StringAttribute>,
            value: impl Into<StringAttribute>,
        },
        picture {},
        pre {},
        progress {
            value: impl Into<StringAttribute>, // TODO: f64 value
            max: impl Into<StringAttribute>, // TODO: f64 value
        },
        q {
            cite: impl Into<StringAttribute>,
        },
        rp {},
        rt {},
        ruby {},
        s {},
        samp {},
        script {
            r#async: impl Into<MaybeDyn<bool>>,
            crossorigin: impl Into<StringAttribute>,
            defer: impl Into<MaybeDyn<bool>>,
            integrity: impl Into<StringAttribute>,
            nomodule: impl Into<MaybeDyn<bool>>,
            nonce: impl Into<StringAttribute>,
            src: impl Into<StringAttribute>,
            script: impl Into<StringAttribute>,
            text: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
        },
        section {},
        select {
            autocomplete: impl Into<StringAttribute>,
            autofocus: impl Into<MaybeDyn<bool>>,
            disabled: impl Into<MaybeDyn<bool>>,
            form: impl Into<StringAttribute>,
            multiple: impl Into<MaybeDyn<bool>>,
            name: impl Into<StringAttribute>,
            required: impl Into<MaybeDyn<bool>>,
            size: impl Into<StringAttribute>, // TODO: int value
            value: impl Into<StringAttribute>,
        },
        small {},
        source {
            src: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
        },
        /// The `<span>` HTML element is a generic inline container for phrasing content, which does not inherently represent anything. It can be used to group elements for styling purposes (using the class or id attributes), or because they share attribute values, such as lang. It should be used only when no other semantic element is appropriate. `<span>` is very much like a `<div>` element, but `<div>` is a block-level element whereas a `<span>` is an inline element.
        span {},
        strong {},
        style {
            media: impl Into<StringAttribute>,
            nonce: impl Into<StringAttribute>,
            title: impl Into<StringAttribute>,
            r#type("type"): impl Into<StringAttribute>,
        },
        sub {},
        summary {},
        sup {},
        table {},
        tbody {},
        td {
            colspan: impl Into<StringAttribute>, // TODO: int value
            headers: impl Into<StringAttribute>,
            rowspan: impl Into<StringAttribute>, // TODO: int value
        },
        template {},
        textarea {
            autocomplete: impl Into<StringAttribute>,
            autofocus: impl Into<MaybeDyn<bool>>,
            cols: impl Into<StringAttribute>, // TODO: int value
            disabled: impl Into<MaybeDyn<bool>>,
            form: impl Into<StringAttribute>,
            initial_value: impl Into<StringAttribute>,
            maxlength: impl Into<StringAttribute>, // TODO: int value
            minlength: impl Into<StringAttribute>, // TODO: int value
            name: impl Into<StringAttribute>,
            placeholder: impl Into<StringAttribute>,
            readonly: impl Into<MaybeDyn<bool>>,
            required: impl Into<MaybeDyn<bool>>,
            rows: impl Into<StringAttribute>, // TODO: int value
            spellcheck: impl Into<MaybeDyn<bool>>,
            r#type("type"): impl Into<StringAttribute>,
            value: impl Into<StringAttribute>,
            wrap: impl Into<StringAttribute>,
        },
        tfoot {},
        th {
            abbr: impl Into<StringAttribute>,
            colspan: impl Into<StringAttribute>, // TODO: int value
            headers: impl Into<StringAttribute>,
            rowspan: impl Into<StringAttribute>, // TODO: int value
            scope: impl Into<StringAttribute>,
        },
        thead {},
        time {
            datetime: impl Into<StringAttribute>,
        },
        title {},
        tr {},
        track {
            default: impl Into<MaybeDyn<bool>>,
            kind: impl Into<StringAttribute>,
            label: impl Into<StringAttribute>,
            src: impl Into<StringAttribute>,
            srclang: impl Into<StringAttribute>,
        },
        u {},
        /// The `<ul>` HTML element represents an unordered list of items, typically rendered as a bulleted list.
        ul {},
        var {},
        video {
            autoplay: impl Into<MaybeDyn<bool>>,
            controls: impl Into<MaybeDyn<bool>>,
            crossorigin: impl Into<StringAttribute>,
            height: impl Into<StringAttribute>, // TODO: int value
            r#loop("loop"): impl Into<MaybeDyn<bool>>,
            muted: impl Into<MaybeDyn<bool>>,
            playsinline: impl Into<MaybeDyn<bool>>,
            poster: impl Into<StringAttribute>,
            preload: impl Into<StringAttribute>,
            src: impl Into<StringAttribute>,
            width: impl Into<StringAttribute>, // TODO: int value
        },
        wbr {},
    }

    impl_svg_elements! {
        svg {
            xmlns: impl Into<StringAttribute>,
        },
        svg_a("a") {},
        animate {},
        animateMotion {},
        animateTransform {},
        circle {},
        clipPath {},
        defs {},
        desc {},
        discard {},
        ellipse {},
        feBlend {},
        feColorMatrix {},
        feComponentTransfer {},
        feComposite {},
        feConvolveMatrix {},
        feDiffuseLighting {},
        feDisplacementMap {},
        feDistantLight {},
        feDropShadow {},
        feFlood {},
        feFuncA {},
        feFuncB {},
        feFuncG {},
        feFuncR {},
        feGaussianBlur {},
        feImage {},
        feMerge {},
        feMergeNode {},
        feMorphology {},
        feOffset {},
        fePointLight {},
        feSpecularLighting {},
        feSpotLight {},
        feTile {},
        feTurbulence {},
        filter {},
        foreignObject {},
        g {},
        hatch {},
        hatchpath {},
        image {},
        line {},
        linearGradient {},
        marker {},
        mask {},
        metadata {},
        mpath {},
        path {},
        pattern {},
        polygon {},
        polyline {},
        radialGradient {},
        rect {},
        svg_script("script") {},
        set {},
        stop {},
        svg_style("style") {},
        switch {},
        symbol {},
        text {},
        textPath {},
        svg_title("title") {},
        tspan {},
        r#use("use") {},
        view {},
    }
}

/// A trait that is implemented for all elements and which provides all the global HTML attributes.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes>
pub trait HtmlGlobalAttributes: SetAttribute + Sized {
    impl_attributes! {
        /// Provides a hint for generating a keyboard shortcut for the current element. This attribute consists of a space-separated list of characters. The browser should use the first one that exists on the computer keyboard layout.
        accesskey: impl Into<StringAttribute>,
        /// Controls whether inputted text is automatically capitalized and, if so, in what manner.
        autocapitalize: impl Into<StringAttribute>,
        /// Indicates that an element is to be focused on page load, or as soon as the `<dialog>` it is part of is displayed. This attribute is a boolean, initially false.
        autofocus: impl Into<MaybeDyn<bool>>,
        /// The class global attribute is a space-separated list of the case-sensitive classes of the element.
        /// Classes allow CSS and JavaScript to select and access specific elements via the class selectors.
        class: impl Into<StringAttribute>,
        /// An enumerated attribute indicating if the element should be editable by the user. If so, the browser modifies its widget to allow editing. The attribute must take one of the following values:
        /// * `true` or the empty string, which indicates that the element must be editable;
        /// * `false`, which indicates that the element must not be editable.
        contenteditable: impl Into<StringAttribute>,
        /// An enumerated attribute indicating the directionality of the element's text. It can have the following values:
        /// * `ltr`, which means left to right and is to be used for languages that are written from the left to the right (like English);
        /// * `rtl`, which means right to left and is to be used for languages that are written from the right to the left (like Arabic);
        /// * `auto`, which lets the user agent decide. It uses a basic algorithm as it parses the characters inside the element until it finds a character with a strong directionality, then it applies that directionality to the whole element.
        dir: impl Into<StringAttribute>,
        /// An enumerated attribute indicating whether the element can be dragged, using the Drag and Drop API. It can have the following values:
        /// * `true`, which indicates that the element may be dragged
        /// * `false`, which indicates that the element may not be dragged.
        draggable: impl Into<StringAttribute>,
        /// Hints what action label (or icon) to present for the enter key on virtual keyboards.
        enterkeyhint: impl Into<StringAttribute>,
        /// Used to transitively export shadow parts from a nested shadow tree into a containing light tree.
        exportparts: impl Into<StringAttribute>,
        /// An enumerated attribute indicating that the element is not yet, or is no longer, _relevant_. For example, it can be used to hide elements of the page that can't be used until the login process has been completed. The browser won't render such elements. This attribute must not be used to hide content that could legitimately be shown.
        hidden: impl Into<MaybeDyn<bool>>,
        /// The id global attribute defines an identifier (ID) which must be unique in the whole document. Its purpose is to identify the element when linking (using a fragment identifier), scripting, or styling (with CSS).
        id: impl Into<StringAttribute>,
        /// A boolean value that makes the browser disregard user input events for the element. Useful when click events are present.
        inert: impl Into<MaybeDyn<bool>>,
        /// Provides a hint to browsers about the type of virtual keyboard configuration to use when editing this element or its contents. Used primarily on `<input>` elements, but is usable on any element while in contenteditable mode.
        inputmode: impl Into<StringAttribute>,
        /// The is global attribute allows you to specify that a standard HTML element should behave like a defined custom built-in element.
        ///
        /// This attribute can only be used if the specified custom element name has been successfully defined in the current document, and extends the element type it is being applied to.
        is: impl Into<StringAttribute>,
        /// The unique, global identifier of an item.
        itemid: impl Into<StringAttribute>,
        /// Used to add properties to an item. Every HTML element may have an `itemprop` attribute specified, where an `itemprop` consists of a name and value pair.
        itemprop: impl Into<StringAttribute>,
        /// Properties that are not descendants of an element with the `itemscope` attribute can be associated with the item using an `itemref`. It provides a list of element ids (not `itemid`s) with additional properties elsewhere in the document.
        itemref: impl Into<StringAttribute>,
        /// `itemscope` (usually) works along with `itemtype` to specify that the HTML contained in a block is about a particular item. `itemscope` creates the Item and defines the scope of the `itemtype` associated with it. `itemtype` is a valid URL of a vocabulary (such as schema.org) that describes the item and its properties context.
        itemscope: impl Into<MaybeDyn<bool>>,
        /// Specifies the URL of the vocabulary that will be used to define `itemprops` (item properties) in the data structure. `itemscope` is used to set the scope of where in the data structure the vocabulary set by `itemtype` will be active.
        itemtype: impl Into<StringAttribute>,
        /// Helps define the language of an element: the language that non-editable elements are in, or the language that editable elements should be written in by the user. The attribute contains one "language tag" (made of hyphen-separated "language subtags") in the format defined in [RFC 5646: Tags for Identifying Languages (also known as BCP 47)](https://datatracker.ietf.org/doc/html/rfc5646). `xml:lang` has priority over it.
        lang: impl Into<StringAttribute>,
        /// A cryptographic nonce ("number used once") which can be used by Content Security Policy to determine whether or not a given fetch will be allowed to proceed.
        nonce: impl Into<StringAttribute>,
        /// A space-separated list of the part names of the element. Part names allows CSS to select and style specific elements in a shadow tree via the `::part` pseudo-element.
        part: impl Into<StringAttribute>,
        /// Used to designate an element as a popover element (see Popover API). Popover elements are hidden via `display: none` until opened via an invoking/control element (i.e. a `<button>` or `<input type="button">` with a popovertarget attribute) or a `HTMLElement.showPopover()` call.
        popover: impl Into<StringAttribute>,
        /// Roles define the semantic meaning of content, allowing screen readers and other tools to present and support interaction with an object in a way that is consistent with user expectations of that type of object. `roles` are added to HTML elements using `role="role_type"`, where `role_type` is the name of a role in the ARIA specification.
        role: impl Into<StringAttribute>,
        /// The slot global attribute assigns a slot in a shadow DOM shadow tree to an element: An element with a slot attribute is assigned to the slot created by the `<slot>` element whose name attribute's value matches that slot attribute's value.
        slot: impl Into<StringAttribute>,
        /// An enumerated attribute defines whether the element may be checked for spelling errors. It may have the following values:
        /// * empty string or `true`, which indicates that the element should be, if possible, checked for spelling errors;
        /// * `false`, which indicates that the element should not be checked for spelling errors.
        spellcheck: impl Into<StringAttribute>,
        /// Contains CSS styling declarations to be applied to the element. Note that it is recommended for styles to be defined in a separate file or files. This attribute and the `<style>` element have mainly the purpose of allowing for quick styling, for example for testing purposes.
        style: impl Into<StringAttribute>,
        /// An integer attribute indicating if the element can take input focus (is focusable), if it should participate to sequential keyboard navigation, and if so, at what position. It can take several values:
        /// * a _negative value_ means that the element should be focusable, but should not be reachable via sequential keyboard navigation;
        /// * `0` means that the element should be focusable and reachable via sequential keyboard navigation, but its relative order is defined by the platform convention;
        /// * a _positive value_ means that the element should be focusable and reachable via sequential keyboard navigation; the order in which the elements are focused is the increasing value of the tabindex. If several elements share the same tabindex, their relative order follows their relative positions in the document.
        tabindex: impl Into<StringAttribute>,
        /// Contains a text representing advisory information related to the element it belongs to. Such information can typically, but not necessarily, be presented to the user as a tooltip.
        title: impl Into<StringAttribute>,
        /// An enumerated attribute that is used to specify whether an element's attribute values and the values of its Text node children are to be translated when the page is localized, or whether to leave them unchanged. It can have the following values:
        /// * empty string or `yes`, which indicates that the element will be translated.
        /// * `no`, which indicates that the element will not be translated.
        translate: impl Into<StringAttribute>,
        /// An enumerated attribute used to control the on-screen virtual keyboard behavior on devices such as tablets, mobile phones, or other devices where a hardware keyboard may not be available for elements that its content is editable (for example, it is an `<input>` or `<textarea>` element, or an element with the `contenteditable` attribute set).
        /// `auto` or an _empty string_, which automatically shows the virtual keyboard when the element is focused or tapped.
        /// `manual`, which decouples focus and tap on the element from the virtual keyboard's state.
        virtualkeyboardpolicy: impl Into<StringAttribute>,
    }
}

/// A trait that is implemented for all SVG elements and which provides all the global SVG
/// attributes.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute>
pub trait SvgGlobalAttributes: SetAttribute + Sized {
    impl_attributes! {
        accentHeight("accent-height"): impl Into<StringAttribute>,
        accumulate: impl Into<StringAttribute>,
        additive: impl Into<StringAttribute>,
        alignmentBaseline("alignment-baseline"): impl Into<StringAttribute>,
        alphabetic: impl Into<StringAttribute>,
        amplitude: impl Into<StringAttribute>,
        arabicForm("arabic-form"): impl Into<StringAttribute>,
        ascent: impl Into<StringAttribute>,
        attributeName("attributeName"): impl Into<StringAttribute>,
        attributeType("attributeType"): impl Into<StringAttribute>,
        azimuth: impl Into<StringAttribute>,
        baseFrequency("baseFrequency"): impl Into<StringAttribute>,
        baselineShift("baseline-shift"): impl Into<StringAttribute>,
        baseProfile("baseProfile"): impl Into<StringAttribute>,
        bbox: impl Into<StringAttribute>,
        begin: impl Into<StringAttribute>,
        bias: impl Into<StringAttribute>,
        by: impl Into<StringAttribute>,
        calcMode("calcMode"): impl Into<StringAttribute>,
        capHeight("cap-height"): impl Into<StringAttribute>,
        class: impl Into<StringAttribute>,
        clipPathUnits("clipPathUnits"): impl Into<StringAttribute>,
        clipPath("clip-path"): impl Into<StringAttribute>,
        clipRule("clip-rule"): impl Into<StringAttribute>,
        color: impl Into<StringAttribute>,
        colorInterpolation("color-interpolation"): impl Into<StringAttribute>,
        colorInterpolationFilters("color-interpolation-filters"): impl Into<StringAttribute>,
        colorProfile("color-profile"): impl Into<StringAttribute>,
        colorRendering("color-rendering"): impl Into<StringAttribute>,
        crossorigin: impl Into<StringAttribute>,
        cursor: impl Into<StringAttribute>,
        cx: impl Into<StringAttribute>,
        cy: impl Into<StringAttribute>,
        d: impl Into<StringAttribute>,
        decelerate: impl Into<StringAttribute>,
        descent: impl Into<StringAttribute>,
        diffuseConstant("diffuseConstant"): impl Into<StringAttribute>,
        direction: impl Into<StringAttribute>,
        display: impl Into<StringAttribute>,
        divisor: impl Into<StringAttribute>,
        dominantBaseline("dominant-baseline"): impl Into<StringAttribute>,
        dur: impl Into<StringAttribute>,
        dx: impl Into<StringAttribute>,
        dy: impl Into<StringAttribute>,
        edgeMode("edgeMode"): impl Into<StringAttribute>,
        elevation: impl Into<StringAttribute>,
        enableBackground("enable-background"): impl Into<StringAttribute>,
        end: impl Into<StringAttribute>,
        exponent: impl Into<StringAttribute>,
        fill: impl Into<StringAttribute>,
        fillOpacity("fill-opacity"): impl Into<StringAttribute>,
        fillRule("fill-rule"): impl Into<StringAttribute>,
        filter: impl Into<StringAttribute>,
        filterUnits("filterUnits"): impl Into<StringAttribute>,
        floodColor("flood-color"): impl Into<StringAttribute>,
        floodOpacity("flood-opacity"): impl Into<StringAttribute>,
        fontFamily("font-family"): impl Into<StringAttribute>,
        fontSize("font-size"): impl Into<StringAttribute>,
        fontSizeAdjust("font-size-adjust"): impl Into<StringAttribute>,
        fontStretch("font-stretch"): impl Into<StringAttribute>,
        fontStyle("font-style"): impl Into<StringAttribute>,
        fontVariant("font-variant"): impl Into<StringAttribute>,
        fontWeight("font-weight"): impl Into<StringAttribute>,
        format: impl Into<StringAttribute>,
        from: impl Into<StringAttribute>,
        fr: impl Into<StringAttribute>,
        fx: impl Into<StringAttribute>,
        fy: impl Into<StringAttribute>,
        g1: impl Into<StringAttribute>,
        g2: impl Into<StringAttribute>,
        glyphName("glyph-name"): impl Into<StringAttribute>,
        glyphOrientationHorizontal("glyph-orientation-horizontal"): impl Into<StringAttribute>,
        glyphOrientationVertical("glyph-orientation-vertical"): impl Into<StringAttribute>,
        glyphRef: impl Into<StringAttribute>,
        gradientTransform("gradientTransform"): impl Into<StringAttribute>,
        gradientUnits("gradientUnits"): impl Into<StringAttribute>,
        hanging: impl Into<StringAttribute>,
        height: impl Into<StringAttribute>,
        href: impl Into<StringAttribute>,
        hreflang: impl Into<StringAttribute>,
        horizAdvX("horiz-adv-x"): impl Into<StringAttribute>,
        horizOriginX("horiz-origin-x"): impl Into<StringAttribute>,
        id: impl Into<StringAttribute>,
        ideographic: impl Into<StringAttribute>,
        imageRendering("image-rendering"): impl Into<StringAttribute>,
        in_: impl Into<StringAttribute>,
        in2: impl Into<StringAttribute>,
        intercept: impl Into<StringAttribute>,
        k: impl Into<StringAttribute>,
        k1: impl Into<StringAttribute>,
        k2: impl Into<StringAttribute>,
        k3: impl Into<StringAttribute>,
        k4: impl Into<StringAttribute>,
        kernelMatrix("kernelMatrix"): impl Into<StringAttribute>,
        kernelUnitLength("kernelUnitLength"): impl Into<StringAttribute>,
        kerning: impl Into<StringAttribute>,
        keyPoints("keyPoints"): impl Into<StringAttribute>,
        keySplines("keySplines"): impl Into<StringAttribute>,
        keyTimes("keyTimes"): impl Into<StringAttribute>,
        lang: impl Into<StringAttribute>,
        lengthAdjust("lengthAdjust"): impl Into<StringAttribute>,
        letterSpacing("letter-spacing"): impl Into<StringAttribute>,
        lightingColor("lighting-color"): impl Into<StringAttribute>,
        limitingConeAngle("limitingConeAngle"): impl Into<StringAttribute>,
        local: impl Into<StringAttribute>,
        markerEnd("marker-end"): impl Into<StringAttribute>,
        markerMid("marker-mid"): impl Into<StringAttribute>,
        markerStart("marker-start"): impl Into<StringAttribute>,
        markerHeight("markerHeight"): impl Into<StringAttribute>,
        markerUnits("markerUnits"): impl Into<StringAttribute>,
        markerWidth("markerWidth"): impl Into<StringAttribute>,
        mask: impl Into<StringAttribute>,
        maskContentUnits("maskContentUnits"): impl Into<StringAttribute>,
        maskUnits("maskUnits"): impl Into<StringAttribute>,
        mathematical: impl Into<StringAttribute>,
        max: impl Into<StringAttribute>,
        media: impl Into<StringAttribute>,
        method: impl Into<StringAttribute>,
        min: impl Into<StringAttribute>,
        mode: impl Into<StringAttribute>,
        name: impl Into<StringAttribute>,
        numOctaves("numOctaves"): impl Into<StringAttribute>,
        offset: impl Into<StringAttribute>,
        opacity: impl Into<StringAttribute>,
        operator: impl Into<StringAttribute>,
        order: impl Into<StringAttribute>,
        orient: impl Into<StringAttribute>,
        orientation: impl Into<StringAttribute>,
        origin: impl Into<StringAttribute>,
        overflow: impl Into<StringAttribute>,
        overlinePosition("overline-position"): impl Into<StringAttribute>,
        overlineThickness("overline-thickness"): impl Into<StringAttribute>,
        panose1("panose-1"): impl Into<StringAttribute>,
        paintOrder("paint-order"): impl Into<StringAttribute>,
        path: impl Into<StringAttribute>,
        pathLength("pathLength"): impl Into<StringAttribute>,
        patternContentUnits("patternContentUnits"): impl Into<StringAttribute>,
        patternTransform("patternTransform"): impl Into<StringAttribute>,
        patternUnits("patternUnits"): impl Into<StringAttribute>,
        ping: impl Into<StringAttribute>,
        pointerEvents("pointer-events"): impl Into<StringAttribute>,
        points: impl Into<StringAttribute>,
        pointsAtX("pointsAtX"): impl Into<StringAttribute>,
        pointsAtY("pointsAtY"): impl Into<StringAttribute>,
        pointsAtZ("pointsAtZ"): impl Into<StringAttribute>,
        preserveAlpha("preserveAlpha"): impl Into<StringAttribute>,
        preserveAspectRatio("preserveAspectRatio"): impl Into<StringAttribute>,
        primitiveUnits("primitiveUnits"): impl Into<StringAttribute>,
        r: impl Into<StringAttribute>,
        radius: impl Into<StringAttribute>,
        referrerPolicy("referrerPolicy"): impl Into<StringAttribute>,
        refX("refX"): impl Into<StringAttribute>,
        refY("refY"): impl Into<StringAttribute>,
        rel: impl Into<StringAttribute>,
        renderingIntent("rendering-intent"): impl Into<StringAttribute>,
        repeatCount("repeatCount"): impl Into<StringAttribute>,
        repeatDur("repeatDur"): impl Into<StringAttribute>,
        requiredExtensions("requiredExtensions"): impl Into<StringAttribute>,
        requiredFeatures("requiredFeatures"): impl Into<StringAttribute>,
        restart: impl Into<StringAttribute>,
        result: impl Into<StringAttribute>,
        rotate: impl Into<StringAttribute>,
        rx: impl Into<StringAttribute>,
        ry: impl Into<StringAttribute>,
        scale: impl Into<StringAttribute>,
        seed: impl Into<StringAttribute>,
        shapeRendering("shape-rendering"): impl Into<StringAttribute>,
        slope: impl Into<StringAttribute>,
        spacing: impl Into<StringAttribute>,
        specularConstant("specularConstant"): impl Into<StringAttribute>,
        specularExponent("specularExponent"): impl Into<StringAttribute>,
        speed: impl Into<StringAttribute>,
        spreadMethod("spreadMethod"): impl Into<StringAttribute>,
        startOffset("startOffset"): impl Into<StringAttribute>,
        stdDeviation("stdDeviation"): impl Into<StringAttribute>,
        stemh: impl Into<StringAttribute>,
        stemv: impl Into<StringAttribute>,
        stitchTiles("stitchTiles"): impl Into<StringAttribute>,
        stopColor("stop-color"): impl Into<StringAttribute>,
        stopOpacity("stop-opacity"): impl Into<StringAttribute>,
        strikethroughPosition("strikethrough-position"): impl Into<StringAttribute>,
        strikethroughThickness("strikethrough-thickness"): impl Into<StringAttribute>,
        string: impl Into<StringAttribute>,
        stroke: impl Into<StringAttribute>,
        strokeDasharray("stroke-dasharray"): impl Into<StringAttribute>,
        strokeDashoffset("stroke-dashoffset"): impl Into<StringAttribute>,
        strokeLinecap("stroke-linecap"): impl Into<StringAttribute>,
        strokeLinejoin("stroke-linejoin"): impl Into<StringAttribute>,
        strokeMiterlimit("stroke-miterlimit"): impl Into<StringAttribute>,
        strokeOpacity("stroke-opacity"): impl Into<StringAttribute>,
        strokeWidth("stroke-width"): impl Into<StringAttribute>,
        style: impl Into<StringAttribute>,
        surfaceScale("surfaceScale"): impl Into<StringAttribute>,
        systemLanguage("systemLanguage"): impl Into<StringAttribute>,
        tabindex: impl Into<StringAttribute>,
        tableValues("tableValues"): impl Into<StringAttribute>,
        target: impl Into<StringAttribute>,
        targetX("targetX"): impl Into<StringAttribute>,
        targetY("targetY"): impl Into<StringAttribute>,
        textAnchor("text-anchor"): impl Into<StringAttribute>,
        textDecoration("text-decoration"): impl Into<StringAttribute>,
        textRendering("text-rendering"): impl Into<StringAttribute>,
        textLength("textLength"): impl Into<StringAttribute>,
        to: impl Into<StringAttribute>,
        transform: impl Into<StringAttribute>,
        transformOrigin("transform-origin"): impl Into<StringAttribute>,
        type_: impl Into<StringAttribute>,
        u1: impl Into<StringAttribute>,
        u2: impl Into<StringAttribute>,
        underlinePosition("underline-position"): impl Into<StringAttribute>,
        underlineThickness("underline-thickness"): impl Into<StringAttribute>,
        unicode: impl Into<StringAttribute>,
        unicodeBidi("unicode-bidi"): impl Into<StringAttribute>,
        unicodeRange("unicode-range"): impl Into<StringAttribute>,
        unitsPerEm("units-per-em"): impl Into<StringAttribute>,
        vAlphabetic("v-alphabetic"): impl Into<StringAttribute>,
        vHanging("v-hanging"): impl Into<StringAttribute>,
        vIdeographic("v-ideographic"): impl Into<StringAttribute>,
        vMathematical("v-mathematical"): impl Into<StringAttribute>,
        values: impl Into<StringAttribute>,
        vectorEffect("vector-effect"): impl Into<StringAttribute>,
        version: impl Into<StringAttribute>,
        vertAdvY("vert-adv-y"): impl Into<StringAttribute>,
        vertOriginX("vert-origin-x"): impl Into<StringAttribute>,
        vertOriginY("vert-origin-y"): impl Into<StringAttribute>,
        viewBox: impl Into<StringAttribute>,
        visibility: impl Into<StringAttribute>,
        width: impl Into<StringAttribute>,
        widths: impl Into<StringAttribute>,
        wordSpacing("word-spacing"): impl Into<StringAttribute>,
        writingMode("writing-mode"): impl Into<StringAttribute>,
        x: impl Into<StringAttribute>,
        xHeight("x-height"): impl Into<StringAttribute>,
        x1: impl Into<StringAttribute>,
        x2: impl Into<StringAttribute>,
        xChannelSelector("xChannelSelector"): impl Into<StringAttribute>,
        xmlBase("xml:base"): impl Into<StringAttribute>,
        xmlLang("xml:lang"): impl Into<StringAttribute>,
        xmlSpace("xml:space"): impl Into<StringAttribute>,
        y: impl Into<StringAttribute>,
        y1: impl Into<StringAttribute>,
        y2: impl Into<StringAttribute>,
        yChannelSelector("yChannelSelector"): impl Into<StringAttribute>,
        zoomAndPan("zoomAndPan"): impl Into<StringAttribute>,
    }
}

/// Attributes that are available on all elements.
pub trait GlobalAttributes: SetAttribute + Sized {
    /// Set attribute `name` with `value`.
    fn attr(mut self, name: &'static str, value: impl Into<StringAttribute>) -> Self {
        self.set_attribute(name, value.into());
        self
    }

    /// Set attribute `name` with `value`.
    fn bool_attr(mut self, name: &'static str, value: impl Into<MaybeDyn<bool>>) -> Self {
        self.set_attribute(name, value.into());
        self
    }

    /// Set JS property `name` with `value`.
    fn prop(mut self, name: &'static str, value: impl Into<MaybeDyn<JsValue>>) -> Self {
        self.set_attribute(name, value.into());
        self
    }

    fn add_event_handler<T: AsRef<web_sys::Event> + JsCast>(
        mut self,
        name: &'static str,
        mut handler: impl FnMut(T) + 'static,
    ) -> Self {
        let scope = use_current_scope(); // Run handler inside the current scope.
        let handler = move |ev: web_sys::Event| scope.run_in(|| handler(ev.unchecked_into()));
        self.set_event_handler(name, handler);
        self
    }

    /// Set an event handler with `name`.
    #[deprecated(
        note = "Use add_event_handler instead. Html events have designated methods, e.g. on:click -> on_click."
    )]
    fn on<E: events::EventDescriptor, R>(
        mut self,
        _: E,
        mut handler: impl EventHandler<E, R>,
    ) -> Self {
        let scope = use_current_scope(); // Run handler inside the current scope.
        let handler = move |ev: web_sys::Event| scope.run_in(|| handler.call(ev.unchecked_into()));
        self.set_event_handler(E::NAME, handler);
        self
    }

    /// Set a two way binding with `name`.
    fn bind<E: bind::BindDescriptor>(mut self, _: E, signal: Signal<E::ValueTy>) -> Self {
        let scope = use_current_scope(); // Run handler inside the current scope.
        let handler = move |ev: web_sys::Event| {
            scope.run_in(|| {
                let value =
                    js_sys::Reflect::get(&ev.current_target().unwrap(), &E::TARGET_PROPERTY.into())
                        .unwrap();
                signal.set(E::CONVERT_FROM_JS(&value).expect("failed to convert value from js"));
            })
        };
        self.set_event_handler(<E::Event as events::EventDescriptor>::NAME, handler);

        self.prop(E::TARGET_PROPERTY, move || signal.get_clone().into())
    }
}

impl<T: GlobalProps> GlobalAttributes for T {}

/// Props that are available on all elements.
pub trait GlobalProps: GlobalAttributes + AsHtmlNode + Sized {
    /// Set the inner html of an element.
    fn dangerously_set_inner_html(mut self, inner_html: impl Into<Cow<'static, str>>) -> Self {
        self.as_html_node().set_inner_html(inner_html.into());
        self
    }

    /// Set the children of an element.
    fn children(mut self, children: impl Into<View>) -> Self {
        self.as_html_node().append_view(children.into());
        self
    }

    /// Set a [`NodeRef`] on this element.
    fn r#ref(mut self, noderef: NodeRef) -> Self {
        if is_not_ssr!() {
            noderef.set(Some(self.as_html_node().as_web_sys().clone()));
        }
        self
    }

    fn spread(mut self, attributes: Attributes) -> Self {
        attributes.apply_self(self.as_html_node());
        self
    }
}
