<div align="center">
  <img src="https://user-images.githubusercontent.com/37006668/210620713-562cda3d-9eaa-40c6-a22a-e5d9bd11b425.svg" width="200" />

  <h1>Sycamore</h1>
</div>
<div align="center">

[![Crates.io](https://img.shields.io/crates/v/sycamore)](https://crates.io/crates/sycamore)
[![docs.rs](https://img.shields.io/docsrs/sycamore?color=blue&label=docs.rs)](https://docs.rs/sycamore)
[![GitHub contributors](https://img.shields.io/github/contributors/sycamore-rs/sycamore)](https://github.com/sycamore-rs/sycamore/graphs/contributors)
[![Discord](https://img.shields.io/discord/820400041332179004?label=discord)](https://discord.gg/vDwFUmm6mU)

</div>

Sycamore is a _reactive_ library for creating web apps in **Rust** and **WebAssembly**.

```rust
#[component]
fn Hello() -> View {
    view! {
        p { "Hello World!" }
    }
}
```

- **Lightning Speed**: Sycamore harnesses the full power of [Rust](https://www.rust-lang.org/) via
  [WebAssembly](https://webassembly.org/), giving you full control over performance.
- **Ergonomic and Intuitive**: Write code that feels natural. Everything is built on
  [reactive primitives](https://sycamore.dev/book/introduction/adding-state) without a cumbersome
  virtual DOM.
- **No JavaScript**: Had enough of JavaScript? So have we. Create apps using Sycamore without
  touching a single line of JS.

## Documentation

Sycamore is extensively documented:

- [The Book](https://sycamore.dev/book/introduction): The Sycamore "Book" will first help guide you
  through basic concepts and create a simple app. It then dwelves into some more advanced topics
  for building more complex apps.
- [API Documentation](https://docs.rs/sycamore): the rustdocs for the `sycamore` crate.

**Still have questions?** Don't hesitate to stop by our friendly
[Discord server](https://discord.gg/vDwFUmm6mU).

## Examples

Sycamore has many examples for your reference in the
[`examples/`](https://github.com/sycamore-rs/sycamore/tree/main/examples) directory. Be sure to
check them out!

### Viewing on `examples.sycamore.dev`

All the examples are hosted under `examples.sycamore.dev/<example_name>` with
`<example_name>` being the name of the example you want to view. For instance, the `todomvc` example
is hosted on
[`examples.sycamore.dev/todomvc`](https://examples.sycamore.dev/todomvc).

### Building Locally

All the examples can also be built locally using [Trunk](https://trunk-rs.github.io/trunk/). For instance, the
following command builds and serves the `todomvc` example:

```bash
cd examples/todomvc
trunk serve
```

Now open up <http://localhost:8080> in your browser to see the example running in action.

## Alternatives?

Don't think Sycamore is for you? Thankfully, there are plenty of alternatives!

- **[SolidJS](https://github.com/solidjs/solid): A declarative, efficient and flexible JavaScript
  library for building user interfaces** <br /> Solid is a JavaScript library which greatly inspired
  Sycamore. Many concepts such as fine-grained reactivity and components as factory functions were
  borrowed from Solid. If you don't mind working with JavaScript (or TypeScript), go check it out!
- **[Leptos](https://github.com/leptos-rs/leptos): Build fast web applications with Rust.** <br />
  Leptos is another Rust library based on the fine-grained reactivity paradigm. Leptos shares many
  similarities with Sycamore and, as of now, has a larger community.
- **[Dioxus](https://github.com/dioxuslabs/dioxus): Fullstack app framework for web, desktop, mobile,
  and more.** <br /> Dioxus uses a VDOM instead of fine-grained reactivity for updating the DOM, although
  it uses fine-grained reactivity for state management. This allows Dioxus to target a wide variety of
  platforms including native and mobile.

## Contributing

- Report issues on our [issue tracker](https://github.com/sycamore-rs/sycamore/issues).
- We love Pull Requests! For more information, check out the
  [section on contributing](https://sycamore.dev/book/contributing) in the docs.

Sycamore would not have been possible without the wonderful contributions from the community. Thank
you!

<a href="https://github.com/sycamore-rs/sycamore/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=sycamore-rs/sycamore" />
</a>

Special thanks to [@nate-sys](https://github.com/nate-sys) for designing the Sycamore logo!
