# Documentation

- [Get started](src/content/docs/start.mdx)
- [Elements](src/content/docs/elements.mdx)
- [Dioxus](src/content/docs/dioxus.mdx)
- [Format and keymaps](src/content/docs/packages.mdx)
- [SSH](src/content/docs/ssh.mdx)
- [Architecture](src/content/docs/architecture.mdx)
- [Roadmap](src/content/docs/roadmap.mdx)

## Website

Minimal Astro, MDX, TypeScript, and Bun documentation setup. Pages build to
static files in `dist/`. Website design and hosting are deferred.

```sh
bun install --frozen-lockfile
bun run dev
bun run build
```

Set `SITE_URL` to the production origin when building for deployment. This enables
canonical sitemap generation. Serve `dist/` with any static host. No deployment
account or domain is configured in this repository.

Guides live in `src/content/docs/`; page layouts and styles live in `src/`.
