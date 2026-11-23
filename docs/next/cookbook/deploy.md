---
title: Deploy
---

# Deploying your app

> Note: This page is currently a stub. Help us write it!

## Static hosting providers

Build the release version of your app:

```bash
cargo sycamore build --release
```

`cargo-sycamore` writes the files you need to serve to `dist/public/`. The
provider guides below start from that folder.

### deed.page

[deed.page](https://deed.page) hosts static files without an account. Pack
`dist/public/` and upload the archive with a single request:

```bash
tar -C dist/public -czf site.tar.gz .
curl -T site.tar.gz "https://deed.page/v1/sites?slug=my-sycamore-app"
```

A single request is limited to 4 MB (the size of the compressed archive). Pick
your own slug; if it is already taken, the request returns `409` and you can try
another one. The response contains the URL of your site
(`https://my-sycamore-app.deed.page/`) and a token. The token is only shown
once, so save it: you need to send it as `Authorization: Bearer <token>` to
update or delete the site later. Because the app is served from the root of its
own subdomain, the default `public-url` of `/` works as is. See the
[deed.page docs](https://deed.page/docs) for larger uploads and the rest of the
API.

## GitHub Pages

## Netlify

## Vercel

## Cloudflare Workers

## fly.io

## Azure

## AWS

## Google Cloud

Missing a service? Contribute to the docs by adding it!
