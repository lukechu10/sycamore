---
title: View DSL
---

# The View DSL

Sycamore uses the `view!` macro as an ergonomic way to create complex user
interfaces.

> Dislike macros? Check out the [builder API](/book/guide/view-builder).

The `view!` macro always produces an expression of type `View`.

The simplest view is an empty view. This can be constructed like this:

```rust
view! {}
```

Equivalently, you can also use:

```rust
View::default()
```

## Elements

Elements have a special terse syntax, as opposed to HTML. You only need to write
the tag name a single time and you don't need a bunch of angle brackets. Text
nodes are just string literals.

These elements can also be nested and contain text nodes.

```rust
view! {
    // An empty div
    div {}
    // A div with a paragraph
    div {
        p { "Hello, world!" }
    }
    // Custom elements also work
    my-custom-element {}
}
```

## Interpolation

Views can contain interpolated values. Anything that implements `Into<View>` can
be used. This includes strings, numbers, signals, functions that return these
types, and other views.

When converting a function into a view, it automatically becomes a dynamic view.
The `view!` macro, however, will do this automatically for you by wrapping any
complex expression in a closure.

```rust
let value = 123;
let signal = create_signal(456);

view! {
    p {
        "Value: " (value)
    }
    p {
        // Equivalent of (move || signal.get() + 1)
        (signal.get() + 1)
    }
}
```

Here is another example where we interpolate views.

```rust
let details = view! {
    div { "Details" }
};

let outer_view = view! {
    h1 { "Why is Rust the best language" }
    (details)
};
```

## Control flow

The `view!` macro supports using `if`/`else` and `match` statements directly
inside the DSL.

```rust
view! {
    if hidden.get() {
        p {
            "Can't see me!"
        }
    } else {
        p {
            "Peek a boo!"
        }
    }
}
```

## Attributes

Attributes (including classes and ids) can also be specified.

```rust
view! {
    p(class="my-class", id="my-paragraph", aria-label="My paragraph", "custom_attribute"="foo")
    button(disabled=true) {
       "My button"
    }
}
```

All attributes are type-checked so that, e.g. the `disabled` attribute only
accepts values of type `bool`. For attributes that are not part of the HTML
spec, you can wrap the attribute name in quotes to create a custom attribute.

### Setting inner html

The special `dangerously_set_inner_html` attribute is used to set an HTML string
as the child of an element. This should generally be avoided if possible because
it is a possible security risk. Never pass user input to this attribute as that
will create an XSS (Cross-Site Scripting) vulnerability.

```rust
view! {
    div(dangerously_set_inner_html="<span>Inner HTML!</span>")

    // DO NOT DO THIS!!!
    div(dangerously_set_inner_html=user_input)
}
```

Interpolating strings directly do not have this problem since that will always
result in a text node, not arbitrary HTML nodes.

### Setting node refs

The special `r#ref` attribute is used to set a `NodeRef` to point at the
element.

```rust
let node = create_node_ref();
view! {
    button(r#ref=node)
}
```

For more details, see [Node Ref](/book/guide/node-ref).

### Properties

Properties are set using the `prop:*` directive.

```rust
view! {
    input(r#type="checkbox", prop:indeterminate=true)
}
```

There are some properties that do not have a matching attribute, such as
`indeterminate` in HTML, and which must be set using the `prop:*` directive.
Other properties such as `value` have
[unintuitive behavior](https://stackoverflow.com/a/7986111/9443288) when using
the attribute version.

### Events

Events are attached using the `on_<event>` attribute.

```rust
view! {
    button(on_click=|_| { /* do something interesting */ }) {
        "Click me"
    }
}
```

### Optional attributes

Stringy attributes can also be optional. To make an attribute optional, simply
pass in an `Option`! For example:

```rust
let attr = create_signal(None::<String>);
view! {
    div(data-attr=attr)
    // Will render <div></div> instead of <div data-attr></div> if attr is None.
}
```

## Components

Calling components from the `view!` macro follows essentially the same syntax as
for elements.

```rust
#[component]
fn Button(class: String) -> View { ... }

view! {
    div {
        Button(class="my-button".to_string())
    }
}
```

Components can also accept `children` just like elements.
