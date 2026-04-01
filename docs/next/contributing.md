---
title: Contributing
subsections:
  - roadmap
---

# Contributing to Sycamore

First, thank you for wanting to contribute! Sycamore is completely free and
open-source and could not be so without the help of contributors like you!

Check out our community's
[Code of Conduct](https://github.com/sycamore-rs/sycamore/blob/main/CODE_OF_CONDUCT.md)
and feel free to say hello on our
[Discord server](https://discord.gg/vDwFUmm6mU) if you like. We have a special
channel dedicated to Sycamore development where you can ask questions and
guidance as well as getting to know other users and contributors of Sycamore in
an informal setting.

The rest of this document will contain:

- The high-level design goals of Sycamore.
- A brief tour of the codebase.
- Making changes to Sycamore.
- Setting up your dev-environment.

## What we are trying to build

Sycamore is a reactive library for creating web apps in Rust and WebAssembly. We
currently have the following goals:

- **Capable**: Offer a complete set of tools for anyone to create webapps in
  Rust.
- **Intuitive**: Try to be as simple and as intuitive as possible by using
  patterns that are natural and easy to reason about.
- **Ergonomic**: Avoid boilerplate code.
- **Fast**: Sycamore apps should require the minimal amount of manual
  optimization. We want as many use-cases to be "fast-enough" by default.
- **Productive**: [Fast compilation times](https://xkcd.com/303/).

## Architecture

This section will give a brief tour of Sycamore's project structure and where to
find what you are looking for.

### Directory structure

- Main crates (including `sycamore`, `sycamore-reactive`, etc...) are in
  `packages/`.
- Integration tests are in `packages/sycamore/tests`.
- Benchmarks are in `packages/tools/bench`.
- Examples are in `examples/`.
- The Sycamore website is developed at
  [sycamore-rs/website](https://github.com/sycamore-rs/website). Every time
  `docs/` is modified, a workflow is triggered that will automatically update
  the website with the latest docs.

### Crates

#### `sycamore`

This is the main crate which is intended to be added to the `Cargo.toml` of a
Sycamore project. This crate re-exports most of the APIs of the other crates.

For now, this crate also includes:

- Easing functions for animations.
- Utilities for using the `renderAnimationFrame` API.

These will eventually be moved into a different crate.

#### `sycamore-macro`

This crate contains all the proc-macro logic for `view!`, `#[component]`, and
`#[derive(Props)]`.

We use [syn](https://docs.rs/syn/latest/syn/),
[quote](https://docs.rs/quote/latest/quote/) and
[proc-macro2](https://docs.rs/proc-macro2/latest/proc_macro2/) for parsing and
quasi-quoting. If you are unfamiliar with implementing proc-macros in Rust,
[this resource](https://github.com/dtolnay/proc-macro-workshop) is a good guide
to get started.

#### `sycamore-reactive`

This is the backbone of Sycamore's reactivity system. This crate can be used
stand-alone without any of the other crates.

The API is mostly inspired by [SolidJS](https://www.solidjs.com/).
Behind-the-scenes, we use an arena with indices to keep track of reactive nodes.
This is what allows us to make everything `Copy`able and `'static`.

Everything created inside the reactivity system is owned by the top-level `Root`
struct. This includes the node arena as well as global state used for tracking
dependencies in memos/effects. Finally, the `Root` is also responsible for
propagating updates through the reactive graph.

[This article](https://dev.to/ryansolid/a-hands-on-introduction-to-fine-grained-reactivity-3ndf)
provides a good introduction to how reactivity is implemented (albeit in JS).
The reactive propagation algorithm takes inspiration from
[this article](https://dev.to/modderme123/super-charging-fine-grained-reactive-performance-47ph),
modified because our reactive system is eager rather than lazy for better
predictability.

#### `sycamore-core`

This crate contains all the core utilities for Sycamore's rendering logic. This
includes:

- Runtime support for components.

This crate is backend-agnostic, meaning that there should be no dependence on
`web-sys` or `wasm-bindgen`. Right now, a lot of the core rendering logic is in
`sycamore-web`. We probably want to move as much as possible over into
`sycamore-core` to make it backend-agnostic.

#### `sycamore-web`

This crate contains all the web specific rendering logic for Sycamore. This
includes, notably, `DomNode`, `HydrateNode`, and `SsrNode`. This also includes
other rendering utilities such as `Keyed`, `Indexed`, `Suspense`, `Transition`,
and more... These are all web-specific for now but some should be moved over
into `sycamore-core`.

#### `sycamore-futures`

A lightweight crate to choose between `tokio` when on the server and
`wasm-bindgen-futures` when on the client.

#### `sycamore-router` and `sycamore-router-macro`

This is an implementation of a SPA router for Sycamore. This will eventually be
moved into a new repository.

## Making changes to Sycamore

We don't have a strict process about making changes to Sycamore but most of the
times, it should look something like this.

1. Create one of the following:
   - [GitHub Discussions](https://github.com/sycamore-rs/sycamore/discussions):
     This is an informal way to propose an idea and to receive community
     feedback. This is a good place to start when proposing a new feature to be
     added to Sycamore.
   - [Issue](https://github.com/sycamore-rs/sycamore/issues): A more formal way
     for us to track features and bugs. Please consider looking for duplicates
     before opening a new issue.
   - [Pull Request](https://github.com/sycamore-rs/sycamore/pulls) (PR for
     short): A request to merge code changes. You are welcome to start with a
     pull request but consider starting with either an Issue or a Discussion
     first for larger changes as we don't want anybody's work to be wasted if
     the PR does not end up being merged. On the other hand, PRs are sometimes
     the most effective way to propose changes. We leave it up to your judgement
     to decide which is most appropriate.
2. Other community members can give reviews and comments. If your PR is ready
   and has not received any reviews, feel free to post your PR on our Discord
   server in the development channel.
3. Once they're satisfied with the proposed changes, they leave the "Approved"
   review. At this point, I ([@lukechu10](https://github.com/lukechu10)) will
   make a final review and merge the PR.

Please make sure that your code is properly formatted using `cargo fmt` and that
is passes `cargo clippy`. If you are contributing a new feature, we ask that you
add some unit tests, and eventually a new example. If you are fixing a bug,
consider adding some regression tests so that we know it won't happen again in
the future!

## Setting up your dev-environment

Begin with creating a fork of the sycamore repository and clone it to your
machine.

```bash
git clone https://github.com/<your account>/sycamore
cd sycamore
```

It is strongly recommended to use the `nightly` channel to develop for Sycamore.
You can use the following to install `nightly` and set it as the default for
this project.

```bash
rustup toolchain add nightly
rustup override set nightly
```

Finally, make sure to install the `wasm32-unknown-unknown` target.

```bash
rustup target add wasm32-unknown-unknown --toolchain nightly
```

### Running tests

Running tests is as simple as running `cargo test --all-features`. This will
automatically run all the tests in all the packages, which could take quite some
time!

If you are working on a specific package, say `sycamore-reactive`, it might be
better to first `cd packages/sycamore-reactive` and run
`cargo test --all-features` in there.

#### WASM tests

For the WASM tests specifically, you will need to have
[`wasm-pack`](https://rustwasm.github.io/wasm-pack/) installed. Then run the
following command and follow the instructions in the console.

```bash
cd packages/sycamore
wasm-pack test --chrome
```

If you want to run the tests in a headless instead, just pass the `--headless`
flag.

#### Macro diagnostics snapshot tests

The proc-macro crates have additional tests for ensuring that we have good
diagnostics for common errors. For this, you will need to install the MSRV
version of Rust (currently 1.94).

```bash
rustup toolchain add 1.94
```

The macro tests are disabled by default since when running on other versions of
Rust, they are usually slightly different and thus would cause a lot of churn.
To enable them, set the `RUN_UI_TESTS` env variable to `true`. In addition, if
you want to overwrite the existing snapshot, set the `TRYBUILD` env variable to
`overwrite`. The final command would look something like:

```bash
RUN_UI_TESTS=true TRYBUILD=overwrite cargo +1.94 test
```

## Adding an example

When adding an example, the following steps should be taken:

1. Add your example to the `examples/` folder. An easy way to create a new
   example is the clone an existing example and change the name in the
   appropriate places (folder name and `Cargo.toml`).
2. Add your example to the list of examples in `examples/README.md` along with a
   small description of what your example demonstrates.

## Writing docs

All the documentation is in the `docs/` folder. This includes the
[Sycamore Book](https://sycamore.dev/book/introduction) as well as blog posts.

## Running benchmarks

We have a very basic benchmark setup in `packages/tools/bench`. Simply run
`cargo bench` in that folder to run the micro-benchmarks.

We also have an implementation for
[js-framework-benchmark](https://github.com/krausest/js-framework-benchmark).
Running this locally, however, is quite a bit more involved. Instead, the
simplest way to run this is to add the "performance" label to your PR which will
trigger the CI to run the benchmark automatically on every commit and post the
result as a comment. Note, however, that the CI can be very noisy and that
measurements should be taken with a grain of salt.
