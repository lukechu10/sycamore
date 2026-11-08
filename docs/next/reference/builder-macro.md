---
title: The Builder Macro
---

# The `#[builder]` macro

`#[builder]` turns a function's arguments into a fluent builder API. Calling the
function creates a builder; setters supply its arguments; `.build()` runs the
original function body and returns its result.

```rust
use sycamore::builder;

#[builder]
fn greeting(name: String, punctuation: Option<char>) -> String {
    format!("Hello, {name}{}", punctuation.unwrap_or('!'))
}

let message = greeting().name("Sycamore".to_string()).build();
assert_eq!(message, "Hello, Sycamore!");
```

This macro is not restricted to components or to functions returning `View`.
`#[component]` uses the same builder machinery, with additional prop conventions
explained below.

## The generated API

For the example above, the generated API has these parts:

- A zero-argument `greeting()` constructor, with the original function's
  visibility, that returns a builder with all required arguments unset.
- A builder type with a consuming setter for each argument, named after that
  argument: `.name(...)` and `.punctuation(...)`.
- An inherent `.build()` method, available when all required arguments have
  been supplied, which returns `String`.
- An implementation of `Build<String>` under the same conditions.

Setters take ownership of the builder and return its updated state. You can call
setters in any order. Creating a builder or setting an argument does not run the
original function body.

The implementation also emits hidden helper items. Generated names such as
`__builder_greeting` are implementation details, not names to use in application
APIs. The examples of generated types on this page are schematic rather than
literal macro expansions.

### Required arguments and type states

By default, an argument is required. Each required argument adds a type-state
parameter to the builder. Initially it is `Missing`; its setter changes that
parameter to `Set<T>`, where `T` is the argument's value type.

For example, ignoring optional arguments and other implementation details:

```rust,ignore
fn greeting() -> GreetingBuilder<Missing>;

impl GreetingBuilder<Missing> {
    fn name(self, name: String) -> GreetingBuilder<Set<String>>;
}

impl GreetingBuilder<Set<String>> {
    fn build(self) -> String;
}
```

Consequently, leaving out a required argument is a compile-time error. A required
setter is available only while its argument is missing, so calling it twice is
also a compile-time error. This is different from optional and defaulted setters,
which can be called repeatedly: the last value wins.

Builders implement `Build<T>`, where `T` is the original function's return type.
This lets APIs accept a builder without requiring the caller to invoke `.build()`
first. For example, component builders implementing `Build<View>` can be passed
directly to a view builder's `.children(...)` method.

> Rust sometimes reports a failed outer `Build<T>` bound instead of the missing
> argument's diagnostic. Calling `.build()` explicitly can give a more useful
> error about which argument is missing.

## Argument options

Options are placed on function arguments, not on the function itself. Multiple
options can share an attribute, such as `#[builder(default, into)]`, or appear in
separate attributes.

### Automatically optional `Option<T>` arguments

An `Option<T>` argument is optional without an attribute:

```rust
#[builder]
fn label(text: String, tooltip: Option<String>) -> String {
    // ...
    text
}

let plain = label().text("Save".to_string()).build();
let with_tooltip = label()
    .text("Save".to_string())
    .tooltip("Save changes".to_string())
    .build();
```

If unset, the function receives `None`. The setter takes **`T`, not `Option<T>`**,
and stores `Some(value)`. There is no automatic `None` setter.

Recognition is syntactic: `Option<T>`, `std::option::Option<T>`, and
`core::option::Option<T>` are recognized. Type aliases are not resolved. An
unqualified `Option` is assumed to mean the standard library type.

`#[builder(required)]` opts out of this automatic behavior. The argument becomes
required, and its setter takes the full `Option<T>`, allowing the caller to
explicitly pass either `Some(value)` or `None`.

### Default values

`#[builder(default)]` makes an argument optional and uses `Default::default()`
when it is unset:

```rust
#[builder]
fn repeat(text: String, #[builder(default)] count: usize) -> String {
    text.repeat(count)
}
```

`#[builder(default = expression)]` instead evaluates the supplied expression:

```rust
#[builder]
fn repeat(
    text: String,
    #[builder(default = 1)] count: usize,
) -> String {
    text.repeat(count)
}
```

Defaults are evaluated at build time, only for arguments that were not set.
Default expressions are evaluated outside the original function body; they
cannot refer to its local variables or its other arguments. They can use the
function's generic parameters and names available in the surrounding scope.

`default` requires the argument type to implement `Default` when building, even
if a setter supplied a value: the build method still has that bound.
`default = expression` does not add a `Default` bound. A setter supplies the
argument's full declared type. In particular, an
`Option<T>` argument with an explicit default has a setter taking `Option<T>`
rather than `T`.

`required` and `default` cannot be combined. For `#[builder]` itself, `required`
is only meaningful on an automatically optional `Option<T>` argument; other
arguments are already required.

### Converting setter inputs

`#[builder(into)]` changes a setter to accept `impl Into<SetterType>`, converting
the supplied value when the setter is called:

```rust
#[builder]
fn greeting(#[builder(into)] name: String) -> String {
    format!("Hello, {name}!")
}

let message = greeting().name("Sycamore").build();
```

`SetterType` is the type the setter would otherwise accept. For an automatically
optional `Option<T>`, it is `T`. With `required` or an explicit default, it is the
full declared type instead.

`into` does not by itself make an argument optional.

## Component props

Use `#[prop(...)]` instead of `#[builder(...)]` on `#[component]` arguments. The
component macro preprocesses props, then passes the function to the builder
machinery.

For props whose type path ends in `Children`, `Attributes`, or `Callback`, the
component macro automatically supplies `#[prop(default, into)]`. This is a
syntactic check, including qualified paths; it does not resolve aliases or
inspect the type behind a name.

For these types, the rules are:

| Explicit options | Result |
| --- | --- |
| None | Automatic `default, into` |
| `required` | Required, with no automatic conversion |
| `required, into` | Required, with an `Into` setter |
| `required, default` | Error: required and default conflict |
| `required, default, into` | The same conflict error |
| `default` or `into` without `required` | Error: the explicit option is unnecessary |

The component macro removes `required` for these special types before builder
expansion. These conventions are specific to `#[component]`: applying
`#[builder]` directly to an argument of one of these types does not make it
optional or enable conversions automatically.

## Generics and type inference

The builder retains the function's generic parameters, bounds, and `where`
clause. Type and const parameters belong to the constructor and builder type,
not to individual setters.

```rust
#[builder]
fn identity<T>(value: T) -> T {
    value
}

let value = identity().value(42_u32).build();
let value = identity::<u32>().value(42).build();
```

The first call infers `T` from the setter argument. The second specifies it on
the constructor. Setters do not each choose a new `T`: if two arguments have the
same generic type, both setters must supply that same type.

Because the constructor has no value arguments, inference relies on later
setters and on how the result is used. If those do not constrain a parameter,
provide a type annotation or explicit constructor arguments. Const generics work
the same way: a setter accepting `[T; N]`, for example, can constrain both `T` and
`N`.

### Argument-position `impl Trait`

An argument declared as `impl Trait` becomes an additional generic type parameter
on the constructor and builder, with the specified trait bounds. Its concrete
type can usually be inferred from the setter:

```rust
#[builder]
fn display_value<T: std::fmt::Display>(
    prefix: T,
    value: impl std::fmt::Display,
) -> String {
    format!("{prefix}: {value}")
}

let text = display_value().prefix("answer").value(42).build();
let text = display_value::<&str, _>()
    .prefix("answer")
    .value(42)
    .build();
```

Unlike an ordinary function call with argument-position `impl Trait`, the
generated constructor exposes that extra type parameter. When using a
turbofish, account for it with an explicit type or `_`.

An optional argument can still leave its generic type unconstrained when omitted:

```rust
#[builder]
fn maybe_display(value: Option<impl std::fmt::Display>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

// The missing value still needs a concrete type implementing Display.
let text = maybe_display::<String>().build();
```

### Builder type parameters

Conceptually, the complete generated builder has parameters in this order:

1. Generated lifetimes for inferred input lifetimes.
2. The original function's generic parameters, in their original order.
3. Generated type parameters for argument-position `impl Trait`, in input order.
4. A result type parameter if the return type is opaque.
5. A type-state parameter for each required argument, in argument order.

The result and state parameters are part of the builder type, not extra generic
arguments callers supply to the constructor. The constructor fixes required
states to `Missing` and, for an opaque return, supplies the hidden result type.

For example, a function with this signature:

```rust
#[builder]
fn choose<T>(value: T, fallback: Option<T>) -> T {
    fallback.unwrap_or(value)
}
```

has a builder shaped like `ChooseBuilder<T, ValueState>`. `fallback` adds no state
parameter because it is optional. `.value(value)` changes `ValueState` from
`Missing` to `Set<T>` without changing `T`.

## Opaque return types

A function returning `impl Trait` still returns an opaque type when built. The
macro does not replace the result with `Box<dyn Trait>` or require it to be
nameable by the caller.

```rust
#[builder]
fn numbers(end: u32) -> impl Iterator<Item = u32> {
    0..end
}

let values: Vec<_> = numbers().end(3).build().collect();
assert_eq!(values, vec![0, 1, 2]);
```

Conceptually, this produces a constructor and builder like:

```rust,ignore
fn numbers() -> NumbersBuilder<impl Iterator<Item = u32>, Missing>;

impl<Output> NumbersBuilder<Output, Missing> {
    fn end(self, end: u32) -> NumbersBuilder<Output, Set<u32>>;
}

impl<Output> NumbersBuilder<Output, Set<u32>> {
    fn build(self) -> Output;
}
```

The builder carries the hidden result type as a generic parameter and stores a
function pointer that produces that result from the completed arguments. It
implements `Build<Output>` for that same hidden type. This does not add dynamic
dispatch or heap allocation for the result.

The original opaque type's bounds and Rust's normal lifetime/capture rules still
apply. The macro does not add `Send` or `'static` requirements.

## Async functions

For `#[builder]` on an `async fn`, constructing the builder and calling setters
are synchronous. `.build()` returns the original function's unboxed future,
rather than awaiting it:

```rust
#[builder]
async fn answer(value: u32) -> u32 {
    value + 1
}

// Inside an async context:
let result = answer().value(41).build().await;
```

The completed builder implements `Build<FutureType>`, not `Build<u32>`. Like any
async function, its body runs when the future is polled. Default expressions,
however, are evaluated when `.build()` supplies the arguments, **before** polling
the future. No executor, boxing, `Send`, or `'static` bound is added by the
builder macro.

These are the semantics of `#[builder]` on an async function. Async
`#[component]` functions additionally use the component runtime's async wrapper;
they are not simply an API for obtaining the original function's future.

## Lifetimes

Builders can hold borrowed arguments. Explicit lifetime parameters are retained:

```rust
#[builder]
fn borrowed<'a>(text: &'a str) -> &'a str {
    text
}

let owned = String::from("hello");
let text = borrowed().text(&owned).build();
```

The generated builder is tied to the supplied borrow, and normal Rust borrow
checking applies. It does not require borrowed inputs to be `'static`.

### Inferred input lifetimes

References with elided input lifetimes are given explicit generated lifetimes on
the builder. Input `'_` lifetimes are likewise given generated names. Each
independently elided input reference generally introduces a distinct lifetime.
For example, the builder for `fn pair(left: &str, right: &str)` can carry separate
lifetimes for `left` and `right`.

Function-pointer types and parenthesized `Fn(...)` signatures retain their own
lifetime-elision scopes. They are not treated as ordinary borrowed fields whose
inner reference lifetimes should be added to the builder.

### Inferred output lifetimes

For an elided output reference, or an output `'_`, the macro can infer a lifetime
only when it finds exactly **one distinct input lifetime** in the input types.
Thus this form is supported:

```rust
#[builder]
fn borrowed(text: &str) -> &str {
    text
}
```

With zero or multiple distinct input lifetimes, write the output lifetime
explicitly. For example, independently elided `left: &str` and `right: &str`
would make `-> &str` ambiguous. Use an explicit relationship instead:

```rust
#[builder]
fn first<'a, 'b>(left: &'a str, right: &'b str) -> &'a str {
    left
}
```

This inference is syntactic, not an analysis of which argument the body returns.
Using one explicit lifetime for multiple inputs also leaves only one distinct
input lifetime. Explicit annotations remain subject to normal Rust lifetime
checking.

## Attributes, scope, and restrictions

Function-level attributes remain on the generated constructor. `cfg`, `allow`,
`warn`, `deny`, and `forbid` also apply to the generated top-level items.
`cfg_attr` is filtered recursively so only those shared attributes are forwarded
to the other items. Other function attributes are not generally copied to every
generated item.

Non-builder attributes on arguments, including doc comments, are forwarded to
the corresponding setter methods, not the private helper's parameters. Such an
attribute must therefore be valid on a method. For example, documentation on an
argument becomes documentation for its setter.

The macro supports safe, non-`const` Rust free functions, including nested
functions. It does not support:

- Methods with a `self` receiver, `unsafe` or `const` functions, explicit ABIs, or
  variadic arguments.
- Destructured arguments, `ref` patterns, or `@` patterns. Arguments must have
  plain names; `mut` is allowed.
- An argument named `build`, which would conflict with the generated method.
- Attributes on generic parameters.
- Inferred `_` types or the never type `!` in the signature.

The generated constructor replaces the original function name. Recursive calls
to that name in the original body are therefore not supported: the name now
refers to the builder constructor, not the original argument-taking function.

The builder and helpers stay in the original scope. Their generated names must
not collide with existing items: for a function `example`, these include
`__builder_example`, `__builder_example_fields`, `__builder_example_inner`, and,
when there are defaults, `__builder_example_defaults`. These names are useful
when reading compiler errors or expansions, but should not be treated as a
stable application-facing API.
