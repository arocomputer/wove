import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://wovetui.com",
  devToolbar: { enabled: false },
  // Guide URLs that were published before the docs were reorganized.
  redirects: {
    "/sidebar": "/",
    "/docs/start": "/docs/quickstart/",
    "/docs/sessions": "/docs/running/",
    "/docs/packages": "/docs/keymap/",
  },
  integrations: [mdx(), sitemap()],
  markdown: {
    shikiConfig: {
      themes: { light: "github-light", dark: "github-dark" },
      // Long lines wrap inside the block instead of scrolling sideways.
      wrap: true,
    },
  },
});
