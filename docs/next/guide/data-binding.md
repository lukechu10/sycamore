---
title: Data Binding
---

# Data Binding

You can bind your `MutSignal` to a JavaScript property with the `bind:` directive.
This will cause your signal value to be synchronised with the property at all
times.

```rust
use sycamore::prelude::*;

let value = create_signal(String::new());

view! {
    input(bind_value=value)
}
```

If the user types into the input field, the `value` signal will automatically be
updated with the latest value. The other way works as well. If you update the
`value` signal, the input field will be updated accordingly.

The way this works is by listening to specific events on the DOM node according
to the property. For instance, `value` uses the `on:input` event.

## Supported properties

Below is a table of supported properties and events that are listened to.

| Property        | Event name | Value type  |
| :-------------- | :--------- | :---------- |
| `value`         | `input`    | `String`    |
| `valueAsNumber` | `input`    | `f64`       |
| `checked`       | `change`   | `bool`      |

Be aware that the `valueAsNumber` property will only work as expected on `input`
elements with type "range" or "number".
