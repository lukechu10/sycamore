---
title: Async Resources
---

# Async Resources and Suspense

In any real non-trivial app, you probably need to fetch data from somewhere.
This is where resources and suspense come in.

The resources API provides a simple interface for fetching and refreshing async
data and suspense makes it easy to render fallbacks while the data is loading.

## The Resources API

To create a new resource, call the `create_isomorphic_resource` function.

```rust
use sycamore::prelude::*;
use sycamore::web::create_isomorphic_resource;

struct Data {
    // Define the data format here.
}

async fn fetch_data() -> Data {
    // Perform, for instance, an HTTP request to an API endpoint.
}

let resource = create_isomorphic_resource(fetch_data);
```

A `Resource<T>` is a wrapper around a `MutSignal<Option<T>>`. The value is
initially set to `None` while the data is loading. It is then set to `Some(...)`
containing the value of the loaded data. This makes it convenient to display the
data in your view.

```rust
view! {
    if let Some(data) = resource.get_clone() {
        ...
    }
}
```

Note that `create_isomorphic_resource`, as the name suggests, runs both on the
client and on the server. If you only want data-fetching to happen on the
client, you can use `create_client_resource` which will never load data on the
server.

Right now, we do not yet have a `create_server_resource` function which only
runs on the server because this requires some form of data-serialization and
server-integration which we have not fully worked out yet.

### Refreshing Resources

Resources can also have dependencies, just like memos. However, since resources
are async, we cannot track reactive dependencies like we would in a synchronous
context. Instead, we have to explicitly specify which dependencies the resource
depends on. This can be accomplished with the `on(...)` utility function.

```rust
let id = create_signal(12345);
let resource = create_client_resource(on(id, move || async move {
    fetch_user(id.get()).await
}));
```

Under the hood, `on(...)` simply creates a closure that first accesses `id` and
then constructs the future. This makes it so that we access the signal
synchronously first before performing any asynchronous tasks.

## Suspense

With async data, we do not want to show the UI until it is ready. This problem
is solved by the `Suspense` component and related APIs. When a `Suspense`
component is created, it automatically creates a new _suspense boundary_. Any
asynchronous data accessed underneath this boundary will automatically be
tracked. This includes accessing resources using the resources API.

Using `Suspense` lets us set a fallback view to display while we are loading the
asynchronous data. For example:

```rust
view! {
    Suspense(fallback=LoadingSpinner) {
        if let Some(data) = resource.get_clone() {
            ...
        }
    }
}
```

Since we are accessing `resource` under the suspense boundary, our `Suspense`
component will display the fallback until the resource is loaded.

## Transition

Resources can also be refreshed when one of its dependencies changes. This will
cause the surrounding suspense boundary to be triggered again.

This is sometimes undesired. To prevent this, just replace `Suspense` with
`Transition`. This component will continue to show the old view until the new
data has been loaded in, providing a smoother experience.
