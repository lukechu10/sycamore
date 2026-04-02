# Examples

## Live Demos

All the examples are hosted under `examples.sycamore.dev/<example_name>` with
`<example_name>` replaced with the name of the example you want to view. For instance, the `todomvc` example
can be viewed at
[`examples.sycamore.dev/todomvc`](https://examples.sycamore.dev/todomvc).

## Building Locally

All the examples can also be built locally using [Trunk](https://trunkrs.dev). For instance, the
following command builds and serves the `todomvc` example:

```bash
cd examples/todomvc
trunk serve
```

Now open up `localhost:8080` in your browser to see the example running locally.

## Example List

| Example                                            | Description                                                                                    |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| [attributes-passthrough](attributes-passthrough)   | Passing through dynamic attributes to an inner element                                         |
| [components](components)                           | UI abstraction using components                                                                |
| [context](context)                                 | Demonstration for the Context API                                                              |
| [counter](counter)                                 | A simple counter which can be incremented and decremented                                      |
| [hello-builder](hello-builder)                     | Hello World! With the builder API!                                                             |
| [hello-world](hello-world)                         | Hello World!                                                                                   |
| [higher-order-components](higher-order-components) | Higher-order-components (functions that create components)                                     |
| [http-request](http-request)                       | Suspense + async components for sending HTTP requests                                          |
| [http-request-builder](http-request-builder)       | Suspense + async components for sending HTTP requests using the builder API!                   |
| [hydrate](hydrate)                                 | Making existing HTML reactive                                                                  |
| [iteration](iteration)                             | Demonstration of how to iterate over data in UI                                                |
| [js-framework-benchmark](js-framework-benchmark)   | Implementation of [js-framework-benchmark](https://github.com/krausest/js-framework-benchmark) |
| [js-snippets](js-snippets)                         | Demonstration of importing a function from JS                                                  |
| [motion](motion)                                   | Demonstration for using animation frames and tweened signals                                   |
| [number-binding](number-binding)                   | Demonstration of binding the value of a range or number input                                  |
| [router](router)                                   | Demonstration of sycamore-router                                                               |
| [ssr](ssr)                                         | Demonstration of server-side-rendering                                                         |
| [ssr-suspense](ssr-suspense)                       | Demonstration of server-side rendering with suspense support                                   |
| [ssr-streaming](ssr-streaming)                     | Demonstration of server-side-rendering with streaming                                          |
| [svg](svg)                                         | Creating SVGs with the `view!` macro                                                           |
| [timer](timer)                                     | Demonstration of using futures to auto-increment a counter                                     |
| [todomvc](todomvc)                                 | Fully compliant implementation of [TodoMVC](https://todomvc.com/) spec                         |
| [transitions](transitions)                         | Suspense + async transitions                                                                   |
