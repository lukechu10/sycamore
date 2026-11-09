---
title: Attribute Passthrough
---

# Attribute Passthrough

Say you're working on a reusable component library for Sycamore. You provide
components such as `Button`, `Container`, and `Grid` etc. which wraps native
HTML elements with extra behavior.

However, a downstream user of your library uses the `Button` component and wants
to set a custom `id` attribute on the wrapped `<button>` element. How do you
solve this?

One way to do this is to add an additional `id` prop to your component and
forward it to the `<button>` element. However, this approach is not scalable. If
we can support setting a custom `id` attribute, why not also support `class`,
`disabled`, etc. as well as all the different possible events.

For such scenarios, **attribute passthrough** comes to the rescue!

## The `attributes` prop

You can add a special `attributes` prop with the `Attributes` type to your
component. You must then specify what kind of element these attributes are
expected to be applied on using the `#[component(extend_element!(...))]` syntax.
Finally, you can **spread** the attributes on the HTML element using the
attribute spread syntax.

```rust
// `html` means that we are spreading onto an HTML element.
// The other possible value is `svg`.
//
// `button` means that we are spreading onto a `<button>` element.
#[component(extend_element!(html:button))]
fn Button(
    attributes: Attributes,
    // We can still accept children.
    children: Children,
    // We can still accept other props besides `attributes`.
    other_prop: i32,
) -> View {
    view! {
        // The spread (`..xyz`) syntax applies all the attributes onto the element.
        button(..attributes)
    }
}
```

## Passing attributes to components

Downstream users of your component can set HTML attributes (and events, props,
etc.) in the same way as they would on an element.

```rust
view! {
    Button(
        // Pass as many HTML attributes/events as you want.
        id="my-button",
        class="btn btn-primary",
        on_click=|_| {}
        // You can still pass in regular props as well.
        other_prop=123,
    ) {
        // Children still gets passed into the `children` prop.
        "Click me"
    }
}
```

Attributes are still fully type checked and will cause compile errors for
unknown attributes or wrong types.

For attributes applied to an element, pass static values directly and use an
explicit closure for reactive expressions, for example
`button(disabled=move || is_disabled.get(), ..attributes)`. Attribute values
passed to a component are ordinary component props: they are not automatically
wrapped in closures, so provide a closure when the receiving prop expects a
dynamic value.

## Intercepting attributes

To intercept an attribute, for instance, to modify it, simply add the attribute
as a regular prop to your component. Say we wanted to add some classes to our
button in addition to any extra classes we get from attribute passthrough. We
could then write something like this.

```rust
use sycamore::web::StringAttribute;

#[component(extend_elements!(html:button))]
fn Button(
    attributes: Attributes,
    // StringAttribute is a type alias for MaybeDyn containing a string.
    class: StringAttribute,
) -> View {
    // We still want the attribute to be reactive.
    let class = move || format!("{class} custom-button");
    view! {
        button(class=class, ..attributes)
    }
}
```

Because of how Rust's method resolution works, setting `class` on our `Button`
component will reference the prop rather then the attribute, which lets us
intercept it.
