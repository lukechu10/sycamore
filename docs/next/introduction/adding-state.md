---
title: Adding State
---

# Adding State

Right now, our app is completely static. We will now see how we can add
interactivity to our app using reactive state.

## Reactivity

> Sycamore's fine-grained reactivity is based on
> [SolidJS](https://www.solidjs.com/). If you've used SolidJS before, a lot of
> this will be very similar.

### Signals

Reactive state is built from reactive nodes. The simplest example of a reactive
node is a **signal**. This can be thought of as a simple wrapper around a type
that can keep track of when it is accessed and when it is updated. To create a
new signal, use the `create_signal` function.

```rust
let signal = create_signal(123);
```

> By convention, functions related to reactive state should be called either
> `create_*` or `use_*` and are called "hooks". We will see many other examples
> of such hooks soon.

The value inside a signal can be read or written. By default, the `MutSignal::get`
method will copy the value inside the signal so that the wrapped type must
implement `Copy`. If you have a non copyable type, you can use
`MutSignal::get_clone` instead.

```rust
let signal = create_signal(123);
// Should print `123`.
console_log!("{}", signal.get());

// Update the signal with a new value.
signal.set(456);

// Should print `456`.
console_log!("{}", signal.get());
// `MutSignal<T>` also implements `Display` so this is the same as the above.
console_log!("{signal}");
```

> We are using `console_log!` here instead of `println!` because on the
> `wasm32-unknown-unknown` target, the Rust standard library does not have
> access to web APIs and so `println!` does not do anything. The `console_log!`
> macro instead calls the `console.log` function which prints the value into the
> JavaScript console.

Signals are also examples of a cell-type in Rust that provides
interior-mutability. What this means concretely is that `MutSignal::set` just takes
a normal (immutable) reference instead of a mutable reference. This allows us to
bypass Rust's XOR mutability restriction so that we can update the signal from
multiple places in our code.

### Effects

Suppose we want to print the signal every time the value changes. A simple way
to do this would be to add a `println!` every time we call `MutSignal::set`.
However, this quickly becomes tedious and error-prone. Instead, we can use
**effects**. Effects are functions that get called every time one of its
dependencies is updated. Dependencies are automatically tracked whenever they
are used from inside the function.

```rust
let signal = create_signal(123);
create_effect(move || {
    // Using `.get(...)` automatically tracks this signal as a dependency.
    let value = signal.get();
    console_log!("{value}");

    // Or we can use the shorter: `console_log!("{signal}");`
});
signal.set(456);
signal.set(789);
```

This prints the following to the terminal:

```
123
456
789
```

Notice how we used the `move` keyword to move our signal into the effect closure
but we can still use it from outside the effect. This is because `MutSignal<T>`
implements `Copy`, even when `T` is not `Copy`. Internally, signals are
allocated on an arena-like structure which stores the value and the `MutSignal<T>`
type is simply an id into this arena, which allows it be `Copy`. The benefit of
this should be obvious: if signals were not copyable, we would need to clone
them every time we used them from inside a closure.

### Memos and derived state

We can also create derived state, that is, state that is a function of other
state. The simplest way to do this is to create a closure. Now, every time we
access our derived signal, we will also automatically track `signal`. So we can
write something like:

```rust
let signal = create_signal(1);
let derived = move || signal.get() * 2;

create_effect(move || {
    let value = signal.get();
    let doubled = derived();
    console_log!("signal = {signal}, doubled = {doubled}");
})
```

Effects automatically de-duplicate dependencies so here, `signal` is only
tracked a single time even though it is accessed twice, once in `signal.get()`,
and another time when calling `derived()`.

However, if instead of simply doubling our value, what if we performed a more
expensive computation. In this case, if we accessed `derived` multiple times, we
would be wasting a lot of work by recalculating our derived state although
`signal` has not changed. We can prevent this by using **memos**, which store
the value of the derived signal and only updates the value when its dependencies
changes.

```rust
let signal = create_signal(1);
let derived = create_memo(move || expensive_computation(signal.get()));

// The memo will only call the closure once so long as `signal` hasn't changed.
let foo = derived.get();
let bar = derived.get();

// This also triggers `derived` to recompute its value.
signal.set(2);
```

The `create_memo` hook returns a writable `MutSignal<T>`. You can override its
stored value with methods such as `.set()` and `.update()`, just like any other
signal. These writes notify dependents but do not change the source signals.
The next dependency-triggered recomputation may overwrite the manual value:

```rust
let signal = create_signal(2);
let doubled = create_memo(move || signal.get() * 2);

doubled.set(100);
assert_eq!(doubled.get(), 100);
assert_eq!(signal.get(), 2);

signal.set(3);
assert_eq!(doubled.get(), 6);
```

If you only want to expose read access, dereference the signal to obtain a
`Signal<T>`. This is also necessary when passing it by value to a function
or component prop that expects a `Signal<T>`:

```rust
let readonly: Signal<i32> = *doubled;
// Or obtain a read-only handle when creating the memo:
let readonly = *create_memo(move || signal.get() * 2);
```

The related `create_selector`, `create_selector_with`, and signal `.map()` APIs
also return writable signals. Selectors avoid notifying dependents when a
recomputed result compares equal to the current stored value, including any
manual override. Direct writes still use ordinary signal notification behavior
and do not check the selector's comparator.

When using `batch`, a pending dependency-triggered recomputation can overwrite a
manual write at the end of the batch, even if the manual write came after the
dependency update.

> Effects are actually implemented using memos. Semantically speaking, this is
> not really correct since we use effects for creating side-effects and memos
> for pure computations. However, implementation wise, an effect is simply a
> memo that does not return a value.

## Reactive views

Now that we have all this reactive machinery in place, let's see how we can use
it to manage our app's state. What we want to do is to store our state in
reactive nodes and then keep the UI automatically in sync with the state.

Sycamore makes this extremely easy. Simply access the reactive state from inside
an interpolated fragment in the `view!` macro and the UI will automatically
update to any state changes.

```rust
let counter = create_signal(1);

view! {
    p { "Count: " (counter) }
}
```

What this does, behind the hood, is essentially something like this:

```rust
let counter = create_signal(1);

let text1 = document().create_text_node("Count: ");
let text2 = document().create_text_node(counter.get().to_string());

create_effect(move || {
    // This effect is automatically run whenever `counter` is changed.
    text2.set_text_content(counter.get().to_string());
});
```

We can of course use derived state in views as well:

```rust
let counter = create_signal(1);
let doubled = create_memo(move || counter.get() * 2);

view! {
    p { "Count: " (counter) }
    p { "Doubled: " (doubled) }
}
```

### Event handlers

Of course, displaying the state in our UI is not all that useful if we can't
update the state. There are many places where you might update the state but the
most common place is probably inside an event handler.

Sycamore makes it easy to add event handlers to your view, using the
`on_<event>` attribute. For example, the following code adds an event handler to
the `click` event and increments the counter by 1.

```rust
view! {
    button(on_click=move |_| counter.set(counter.get() + 1)) { "Increment" }
    p { "Count: " (counter) }
}
```

Many times, we want to keep the view code on the simpler side so we can extract
the event handler into a local variable:

```rust
let increment = move |_| counter.set(counter.get() + 1);
view! {
    button(on_click=increment) { "Increment" }
}
```

A nice trick is that `MutSignal` actually implements `std::ops::AddAssign` which
lets us write:

```rust
// Notice the `mut` here. This is required because of the `AddAssign` trait.
let mut counter = create_signal(1);
let increment = move |_| counter += 1;
```

Try this out! The complete code for this example is:

```rust
use sycamore::prelude::*;

#[component]
fn App() -> View {
    let mut counter = create_signal(1);
    let doubled = create_memo(move || counter.get() * 2);
    let increment = move |_| counter += 1;

    view! {
        button(on_click=increment) { "Increment" }
        p { "Count:" (counter) }
        p { "Doubled: " (doubled) }
    }
}

fn main() {
    sycamore::render(App);
}
```

Now every time we click on the button, the text displaying the counter will
automatically update.

### Conditional views

Instead of just displaying the state as text on screen, we can also display part
of the UI depending on whether some reactive condition is fulfilled.

```rust
// Create a new bool signal representing whether we show this part of the UI or not.
let show = create_signal(true);

view! {
    if show.get() {
        p { "Now you see me" }
    } else {
        p { "Now you don't" }
    }
}
```

### Reactive components

As we saw before, components in Sycamore are just regular Rust functions. How
can we pass reactive state to components? Simple! Just wrap the prop inside a
`Signal` (or a `MutSignal` if you need to mutate the value inside the
component). For example, we can write:

```rust
#[component]
fn CounterDisplay(value: Signal<i32>) -> View {
    view! {
        div {
            p { "Counter value: " (value) }
        }
    }
}

let counter = create_signal(1);

view! {
    CounterDisplay(value=*counter)
}
```

However, this is not as flexible as it could be. We can pass signals and memos,
but we cannot pass, for instance, a simple derived closure. To fix this
limitation, you can use the `MaybeDyn<T>` prop instead.

```rust
// `#[prop(into)]` let's us use anything that implements `Into<MaybeDyn<T>>`.
// This includes both `MutSignal` and `Signal` as well as functions.
#[component]
fn CounterDisplay(#[prop(into)] value: MaybeDyn<i32>) -> View { ... }
```
