---
title: SSR Streaming
---

# SSR Streaming

Not only does Sycamore support server side rendering, Sycamore also supports
server side streaming. What this means is that Sycamore can start sending HTML
over to the client before all the data has been loaded, making for a better user
experience.

## Different SSR modes

There are three SSR rendering modes. Blocking and streaming modes require
Sycamore's `suspense` feature, which is enabled by default. If you disable
Sycamore's default features, enable `suspense` explicitly to use either mode.

### Sync mode

This is the default mode and is used when calling `render_to_string`.

In sync mode, data is never fetched on the server side and the suspense fallback
is always rendered.

**Advantages**:

- Fast time-to-first-byte (TTFB) and first-contentful-paint (FCP), since we
  don't need to perform any data-fetching.
- Simpler programming model, since we don't need to worry about data-fetching on
  the server.

**Disadvantages**:

- Actual data will likely take longer to load, since the browser needs to first
  start running the WASM binary before figuring out that more HTTP requests are
  required.
- Worse SEO since data is only loaded after the initial HTML is rendered.

### Blocking mode

The server already knows which data is needed for rendering a certain page. So
why not just perform data-fetching directly on the server? This is what blocking
mode does.

You can use blocking mode by calling `render_to_string_await_suspense`. Blocking
mode means that the server will wait for all suspense to resolve before sending
the HTML.

**Advantages**:

- Faster time to loading data, since the server does all the data-fetching.
- Better SEO since content is rendered to static HTML.

**Disadvantages**:

- Slow time-to-first-byte (TTFB) and first-contentful-paint (FCP) since we must
  wait for data to load before we receive any HTML.
- Slightly higher complexity since we must worry about serializing our data to
  be hydrated on the client.

### Streaming mode

Streaming mode is somewhat of a compromise between sync and blocking mode. In
streaming mode, the server starts sending HTML to the client immediately, which
contains the fallback shell. Once data loads on the server, the new HTML is sent
down the same HTTP connection and automatically replaces the fallback.

You can use streaming mode by calling `render_to_string_stream`.

**Advantages**:

- Fast time-to-first-byte (TTFB) and first-contentful-paint (FCP) since the
  server starts streaming HTML immediately.
- Better SEO for the same reasons as blocking mode.

**Disadvantages**:

- Slightly higher complexity since we must worry about serialiing our data to be
  hydrated on the client, similarly to blocking mode.
