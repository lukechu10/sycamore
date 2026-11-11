---
title: Contexts
---

# Contexts

Contexts provide an easy way to share data between components without drilling
props through multiple levels of the component hierarchy.

It is common to use the
[new type idiom](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)
as the type of the context value. Suppose we want to create a global dark mode
state. We could then define the following `DarkMode` struct.

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
struct DarkMode(MutSignal<bool>);

impl DarkMode {
    fn is_enabled(self) -> bool {
        self.0.get()
    }

    fn toggle(self) {
        self.0.set(!self.0.get());
    }
}
```

## Providing contexts

To make a context value accessible in child components, use the
`provide_context` function.

```rust
let dark_mode = DarkMode(create_signal(false));
provide_context(dark_mode);
```

## Using contexts

Once the context has been provided, it can be used in any nested scope including
from the same scope where the context value was provided.

To access the context, use the `use_context` method.

```rust
#[component]
fn ChildComponent() -> View {
    let dark_mode = use_context::<DarkMode>();
    // ...
}

provide_context::<DarkMode>(...);
view! {
    ChildComponent {}
}
```

The context value is not reactive by itself. If you want to make a context state
reactive, make sure to wrap it inside a signal.
