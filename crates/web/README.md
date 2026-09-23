# Website

The Astro site at https://wovetui.com. It uses Bun and is not part of the Cargo
workspace.

```sh
bun install --frozen-lockfile
bun run dev
bun run build
```

Pushes to `main` that change the site build and deploy it through GitHub
Actions. See [website deployment](../../CONTRIBUTING.md#website) for hosting.

## Where things live

| Path | Contents |
| --- | --- |
| `src/content/docs/` | Guides, one MDX file per page |
| `src/components/Frame.astro` | Shows a PTY frame golden from `scripts/ui/frames/` |
| `src/layouts/Docs.astro` | Sidebar groups and page order |
| `src/pages/` | Homepage, docs index, and crates page |
| `src/styles/site.css` | All styles |
| `public/logo/` | Logos used by the repository READMEs |

The homepage feature list comes from the first bullet list in the root
`README.md`.

To add a guide, create the MDX file and add its id, such as `elements`, to a
group in `Docs.astro`. Example screens come from the reviewed `./x ui` frames,
so they change with the goldens.

The favicon follows the browser's theme. `favicon.svg` switches colors itself;
browsers without SVG favicons get `favicon.ico` on light themes and
`favicon-dark.ico` on dark ones. Regenerate both from `crates/web/` with
ImageMagick:

```sh
magick -size 16x16 xc:none -fill '#171717' -draw 'rectangle 5,3 10,12' -filter point -define icon:auto-resize=48,32,16 public/favicon.ico
magick -size 16x16 xc:none -fill '#EDEDED' -draw 'rectangle 5,3 10,12' -filter point -define icon:auto-resize=48,32,16 public/favicon-dark.ico
```
