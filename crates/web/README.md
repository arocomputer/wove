# Documentation

- [Get started](src/content/docs/start.mdx)
- [Elements](src/content/docs/elements.mdx)
- [Dioxus](src/content/docs/dioxus.mdx)
- [Format and keymaps](src/content/docs/packages.mdx)
- [SSH](src/content/docs/ssh.mdx)
- [Architecture](src/content/docs/architecture.mdx)

## Website

Astro, MDX, TypeScript, and Bun website with a compact homepage and documentation.
Pages use locally served JetBrains Mono Variable, light and dark themes, and mobile navigation. They build to
static files in `dist/` for GitHub Pages at https://wovetui.com.
Shared styles use 16px, 500-weight body text, 24px page titles, 20px section
headings, and 14px code and secondary text. The homepage tagline uses the body scale.

```sh
bun install --frozen-lockfile
bun run dev
bun run build
```

Pushes to `main` check, build, and deploy the site through GitHub Actions.
See [website deployment](../../CONTRIBUTING.md#website) for hosting and DNS setup.

Guides live in `src/content/docs/`; page layouts and styles live in `src/`.
Documentation uses a two-column desktop layout with guide navigation and the
article. Start, Build, and Reference navigation groups are defined in
`src/layouts/Docs.astro`. Narrow screens collapse guide navigation.
The `/crates/` page introduces the four library crates with guide and source links.
Its entries live in `src/pages/crates.astro`.
The homepage centers its content beside a top-aligned left sidebar and uses
`src/components/Overview.astro`. `src/components/Sidebar.astro` supplies the
homepage navigation and stacks above the content on small screens.
The homepage feature list is read from the repository README's first bullet list.
The former `/sidebar/` preview redirects to `/`.
External links open in new tabs; internal navigation stays in the current page.
Only the homepage has a footer. Mobile docs navigation starts collapsed
to avoid refresh jumps.
The website wordmark is inline SVG in `src/components/Wordmark.astro`, with
system-theme CSS included in the markup to avoid an image swap during loading.
The standalone logos in `public/logo/` are used by repository READMEs.
The shared page preloads the Latin font used throughout the site to reduce text
reflow during navigation. The local Astro toolbar is disabled so it does not cover
article text or page navigation.

The favicon is a 6 × 10 cursor block centered in a transparent 16 × 16 SVG,
with system-theme colors and a transparent ICO
fallback. Safari pinned tabs use a separate monochrome mask. Browser-managed
Favorites and Start Page tiles may add their own background.

Regenerate the ICO from this shape using ImageMagick, from `crates/web/`:

```sh
magick -size 16x16 xc:none -fill '#EDEDED' -draw 'rectangle 5,3 10,12' -filter point -define icon:auto-resize=48,32,16 public/favicon.ico
```
