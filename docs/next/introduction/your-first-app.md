---
title: Your First App
---

# Creating your first Sycamore app

This section will guide you through creating your first Sycamore app. We'll
start by introducing the basics such as how to create views, how to manage state
using reactivity, and how rendering lists work. This will build up towards
eventually creating a simple todo app.

## Prerequisites

### Install Rust

First, you'll need to [install Rust](https://www.rust-lang.org/tools/install).
If you have already done this, you can skip to the next section. It is strongly
recommended to use the `rustup` tool to manage your Rust installation as it
makes it very easy to install additional targets and toolchains. In fact, we
will use it right now to install the `wasm32-unknown-unknown` target, which is
needed to compile Rust to WebAssembly.

```bash
rustup target add wasm32-unknown-unknown
```

The minimum supported Rust toolchain is `v1.94`. Sycamore is not guaranteed to
(and probably won't) compile on older versions of Rust.

Sycamore also only works on Rust edition 2021 or later (which should be the
default in new Rust projects). Even though most crates written in edition 2021
are backward compatible with older editions, this is not the case for Sycamore
because Sycamore's proc-macro generates code that is only compatible with
edition 2021. Furthermore, the
[disjoint capture in closures](https://blog.rust-lang.org/2021/10/21/Rust-1.56.0.html#disjoint-capture-in-closures)
feature greatly improves the ergonomics when working with Sycamore's reactivity.

### Install Trunk

[Trunk](https://trunkrs.dev) is the recommended build tool for Sycamore. If you
are familiar with JavaScript frontend development, Trunk is like
[webpack](https://webpack.js.org/) or [rollup](https://rollupjs.org/) but with
first-class support for building Rust WASM apps.

> We are currently working on building a Sycamore specific CLI tool but it is
> not yet ready. For now, Trunk is the recommended to build your Sycamore app.

You can use one of the following commands to install Trunk on your system:

```bash
# Install via homebrew on Mac, Linux or Windows (WSL).
brew install trunk

# Install a release binary (great for CI).
# You will need to specify a value for ${VERSION}.
wget -qO- https://github.com/thedodd/trunk/releases/download/${VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-

# Install via cargo.
cargo install --locked trunk
```

For more information, check out the [Trunk website](https://trunkrs.dev)

### Setting up your IDE

There are lots of options for developing Rust. Some popular options include
[Visual Studio Code](https://code.visualstudio.com/) and
[Rust Rover](https://www.jetbrains.com/rust/). In general, any IDE that supports
LSP (Language Server Protocol) should work fine along with
[Rust Analyzer](https://rust-analyzer.github.io/).

## Create a new Sycamore project

If you want to start with a template project, check out
[`sycamore-start`](https://github.com/sycamore-rs/start), a simple bare-bones
Sycamore template. Or just follow along the instructions.

Now with all the prerequisites installed, we can create our new Sycmore app.

```bash
cargo new hello-sycamore
cd hello-sycamore
```

This creates a blank Rust project called `hello-sycamore`. Let's add Sycamore as
a dependency by running:

```bash
cargo add sycamore@0.9.2
```

You can also do this manually by adding the following line to your `Cargo.toml`
file.

```toml
sycamore = "0.9.2"
```

## Hello, world!

Open up your `src/main.rs` file and replace the contents with:

```rust
use sycamore::prelude::*;

fn main() {
    sycamore::render(|| "Hello, world!".into());
}
```

As you can tell, Sycamore is very focused on reducing any boilerplate. A hello
world app in Sycamore really just is a single line (if we don't count the `use`
statements). Hopefully, what this code does should be somewhat obvious. We start
with a call to `sycamore::render` which initializes our app. This function
expects a closure which returns a `View`, a type that represents our UI tree.
Here, we create a `View` containing a single text node by using `From<&str>`.

Finally, create a `index.html` file at the root of your project directory with
the following contents which is required for Trunk to build your app to
WebAssembly.

```html
<!DOCTYPE html>
<html>
    <head></head>
    <body></body>
</html>
```

All that's left to do is to run:

```bash
trunk serve
```

This will build your app and automatically rebuild it whenever you make a change
to your source file, which is very useful for development. Open up
<http://localhost:8080> in your web browser. If you see the words "Hello,
world!" on the screen, then congratulations! You have successfully created your
first Sycamore app!

### Creating views

On the web, Sycamore renders to HTML. Right now, we are just rendering a single
string. Let's try something slightly more complicated. To do so, we can use the
`view!` macro (as we'll see later, Sycamore also supports using a builder API to
describe views. However, we'll keep it simple for now by sticking to the macro).

Replace the body of your `main` function with:

```rust
sycamore::render(|| view! {
    h1 { "Hello, world!" }
    p { "This is my first Sycamore app" }
});
```

The `view!` macro provides a convenient and concise way of describing UI nodes.
With this example, we are creating the following HTML structure:

```html
<h1>Hello, world!</h1>
<p>This is my first Sycamore app</p>
```

Of course, we can also nest elements, for example, surrounding our elements in a
`<div>...</div>` container.

```rust
view! {
    div {
        h1 { "Hello, world!" }
        p { "This is my first Sycamore app" }
    }
}
```

### Adding attributes

We can also add HTML attributes to our elements using the following syntax:

```rust
view! {
    h1(id="hello-world") { "Hello, world!" }
}
```

This generates the following HTML:

```html
<h1 id="hello-world">Hello, world!</h1>
```

## Using components

Obviously, we don't want to write our entire app inside a single function. To
solve this problem, we can use components. It is conventional to define a
top-level `App` component which acts as the entry-point for your app.

In Sycamore, components are just functions that return `View`. To define a
component, use the `#[component]` attribute.

```rust
#[component]
fn App() -> View {
    view! {
        div {
            h1 { "Hello, world!" }
            p { "This is my first Sycamore app" }
        }
    }
}

fn main() {
    sycamore::render(App);
}
```

We can create a component with the `view!` macro using essentially the same
syntax we have used for elements.

```rust
#[component]
fn HelloWorld() -> View {
    view! {
        p { "Hello, world!" }
    }
}

#[component]
fn App() -> View {
    view! {
        // Components can be at the top-level of a view.
        HelloWorld()

        // Or they can be nested.
        div {
            HelloWorld()
        }
    }
}
```

Components not only help you split up your code, but also let's you reuse code
that is used multiple times in your app.

### Component props

Components really shine with component props. These are arguments to the
component that can affect what is rendered.

To have a component take props, we need to first create a struct reprenseting
the props type.

```rust
#[derive(Props)]
struct HelloProps {
    name: String,
}

#[component]
fn Hello(props: HelloProps) -> View {
    view! {
        p { "Hello, " (props.name) "!" }
    }
}
```

Here, we have defined a `Hello` component that takes in a single prop `name` of
type `String`. We also used a new piece of syntax from `view!`: when surrounding
an expression inside parentheses, the value is interpolated into the view.

We can now call our component like this, which would then display "Hello,
Sycamore!".

```rust
view! {
    Hello(name="Sycamore".into())
}
```

Note that components are fully type-checked, meaning that this will not compile:

```rust
view! {
    // Error: missing prop `name`.
    Hello()
}
```

### Component children

There is a special component prop called `children`. This prop allows you to
nest views inside of a component. For example, suppose we wanted to create a
component that wraps its content inside a `<div>` element. We can easily do that
with:

```rust
#[derive(Props)]
struct WrapperProps {
    children: Children,
}

#[component]
fn Wrapper(props: WrapperProps) -> View {
    view! {
        div {
            (props.children)
        }
    }
}

#[component]
fn App() -> View {
    view! {
        Wrapper {
            p { "Nested children" }
        }
    }
}
```

### Inline props

Creating a new struct every time we want to have a component accept props can
quickly get tedious. To solve this, we can use inline props. If we replace
`#[component]` with `#[component(inline_props)]`, we can write our props
directly as parameters of our component function.

This following example is equivalent to our previous example. What inline props
does behind the hood, in fact, is simply generate the struct for us so that we
don't have to write it out explicitly.

```rust
#[component(inline_props)]
fn Wrapper(children: Children) -> View {
    view! {
        div {
            (children)
        }
    }
}
```

It will likely turn out that you will almost never use struct props, simply
because it involves more typing than inline props. For this reason, we are
considering making inline props the default in a future version of Sycamore.
However, as of v0.9, you must still explicitly annotate your component with
`inline_props`.

Next, we will see how to add state to our app and make it interactive.
