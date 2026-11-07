---
title: JS Interop
---

# JS Interop

Sycamore is built on [`wasm-bindgen`](https://wasm-bindgen.github.io/wasm-bindgen/),
which lets Rust and JavaScript call each other. You can use it directly in your
Sycamore app, along with `js-sys` for JavaScript's built-in objects and `web-sys`
for browser APIs. No Sycamore-specific bindings are needed.

## Calling JavaScript with `wasm-bindgen`

Use `wasm-bindgen` when you need bindings to your own JavaScript code or a
third-party JavaScript library. Add it to your `Cargo.toml`:

```toml
[dependencies]
wasm-bindgen = "0.2"
```

The `#[wasm_bindgen]` attribute lets you declare JavaScript functions that you
can call from Rust. For example, this imports the browser's global `alert`
function:

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
unsafe extern "C" {
    fn alert(message: &str);
}

// Call this from an event handler, for example.
fn say_hello() {
    alert("Hello from Rust!");
}
```

You can also use `#[wasm_bindgen]` on Rust functions to make them callable from
JavaScript. `wasm_bindgen::JsValue` represents a JavaScript value when you need
to pass values that do not have a concrete Rust type.

See the [`wasm-bindgen` guide](https://wasm-bindgen.github.io/wasm-bindgen/)
for details on importing JavaScript modules, exporting Rust functions, passing
values, and managing callbacks.

## JavaScript built-ins with `js-sys`

The `js-sys` crate provides ready-made bindings to standard JavaScript objects
such as `Array`, `Date`, `Map`, and `Promise`. Use these instead of writing your
own bindings. Add `js-sys = "0.3"` to your dependencies, then call its APIs from
Rust:

```rust
let timestamp_ms = js_sys::Date::now();
let random_number = js_sys::Math::random();
```

`js-sys` covers JavaScript itself, not browser-specific APIs such as the DOM.
See the [`js-sys` documentation](https://docs.rs/js-sys/) for the available
bindings.

## Browser APIs with `web-sys`

The `web-sys` crate provides bindings to browser APIs, including the DOM,
local storage, and canvas. Most APIs are behind Cargo feature flags, so enable
the features listed in the documentation for the types and methods you use.
For example, to set the page title:

```toml
[dependencies]
web-sys = { version = "0.3", features = ["Window", "Document"] }
```

```rust
use sycamore::prelude::*;

on_mount(|| {
    let window = web_sys::window().expect("no browser window");
    let document = window.document().expect("no document");
    document.set_title("Hello from Sycamore!");
});
```

Browser APIs are not available during server-side rendering. Call them from
`on_mount` or event handlers rather than during server rendering. If you need
to access an element created by your view, use a [node ref](/book/guide/node-ref) instead
of searching for it in the document.

See the [`web-sys` documentation](https://docs.rs/web-sys/) for available APIs
and their required features.
