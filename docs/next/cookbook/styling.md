---
title: Styling with CSS
---

# Styling with CSS

Styling is definitely important for a modern web app. Sycamore is not
opinionated when it comes to styling and lets you use whichever method works
best for you. Below are some common options.

## Raw CSS files

The simplest is just to use raw CSS files. This is very simple when using Trunk.
For more information, refer to
[the Trunk docs](https://trunk-rs.github.io/trunk/guide/assets/index.html#css). Be sure to include the
`data-trunk` attribute in your `<link>` tag, otherwise Trunk will not serve it.

## CSS framework

Using a CSS framework is also definitely possible with Sycamore. Simply serve
the framework's CSS code using Trunk and you should be able to use the CSS
classes like normal from your Sycamore code.

## Tailwind CSS

One particular kind of CSS frameworks worthy of note are utility-first CSS
frameworks (such as [Tailwind CSS](https://tailwindcss.com/) or
[Windi CSS](https://windicss.org/)).

Trunk has [built-in support](https://trunk-rs.github.io/trunk/guide/assets/index.html#tailwind) for Tailwind
CSS.

## Relevant Examples

- **Sycamore + Trunk + Tailwind CSS**:
  <https://github.com/yerlaser/sycamore_tailwindcss_template>
