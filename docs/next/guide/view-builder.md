---
title: View Builder API
---

# The View Builder API

For those that prefer not to use macros, Sycamore also provides an ergonomic
builder API for composing views.

Start by importing all of the builder HTML tags.

```rust
use sycamore::web::tags::*;
```

## Elements

Elements can easily be created by calling the corresponding function for the
HTML tag.

```rust
a()
button()
div()
// etc...
```

These functions return specific element types such as `HtmlButtonElement`. To
convert those into `View`s, simply use `.build()`. For example, if you are
composing a component, you could write something like:

```rust
#[component]
fn Button() -> View {
    button().build()
}
```

### Children

You can add children to a element by using the `.children(...)` method. Anything
that implements `Build<View>` can be appended as a child node. This includes
string literals, numbers, other elements, signals, functions, and views.

```rust
div().children(
    p().children("Hello World!")
)
```

Tuples also implement `Build<View>` which lets you add multiple children in one
shot.

```rust
div().children((
    span().children("Hello "),
    span().children("World!"),
))
```

### Dynamic views

To add dynamic content, you can pass a function to `.children(...)`. This will
automatically make the view dynamic and will trigger a re-render whenever a
dependency changes.

```rust
let state = create_signal(0);

p().children((
    "value = ",
    state,
    ", doubled = ",
    move || state.get() * 2,
))
```

## Attributes

Every attribute can be set on an element via a method of the same name. Custom
attributes (that are not part of the HTML spec or are `data-*` or `aria-*`
attributes) can be set using `.attr(...)`.

```rust
p().class("my-class").id("my-paragraph").attr("aria-label", "My paragraph")
```

### Setting inner html

The inner HTML can be set using the special `.dangerously_set_inner_html(...)`
method. This should generally be avoided if possible because it is a possible
security risk. Never pass user input to this attribute as that will create an
XSS (Cross-Site Scripting) vulnerability.

```rust
div().dangerously_set_inner_html(inner_html)
```

### Setting node refs

A node ref can be attached using the special `r#ref(...)` method.

```rust
let node = create_node_ref();
button().r#ref(node)
```

For more details, see [Node Ref](/book/guide/node-ref)

### Properties

Properties are set using the `.prop(...)` method.

```rust
input().r#type("checkbox").prop("indeterminate", true)
```

There are some properties that do not have a matching attribute, such as
`indeterminate` in HTML, and which must be set using the `prop:*` directive.
Other properties such as `value` have
[unintuitive behavior](https://stackoverflow.com/a/7986111/9443288) when using
the attribute version.

### Events

Events are attached using `.on_<event>(...)`.

```rust
button().on_click(|_| console_log!("clicked!")).children("Click me!")
```

### Optional attributes

Stringy attributes can also be optional. To make an attribute optional, simply
pass in an `Option`! For example:

```rust
let attr = create_signal(None::<String>);
div().attr("data-attr", attr)
// Will render <div></div> instead of <div data-attr></div> if attr is None.
```

## Components

Components that are generated using the `#[component]` attribute macro look very
similar to built-in elements.

The component builder implements `Build<View>` and so can be inserted directly
under a `.children(...)` call without calling `.build()` explicitly.

> Unfortunately due to Rust trait resolution limitations, error messages are
> degraded when `.build()` is not explicitly used. If you get an error about
> missing props and want to find out more information on which specific prop is
> missing, inserting an explicit `.build()` call can greatly improve error
> messages.

```rust
#[component]
fn Button(class: String) -> View { ... }

div().children(Button().class("my-button".to_string()))
```
