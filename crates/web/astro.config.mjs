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
  vite: {
    build: {
      rolldownOptions: {
        onwarn(warning, warn) {
          // Astro's generated module exports __astroPropagation and its assets.
          // This marker has no JS runtime semantics; Rolldown may safely drop it.
          if (
            warning.code === "MODULE_LEVEL_DIRECTIVE" &&
            warning.message.includes('"use astro:head-inject"') &&
            warning.id?.includes("?astroPropagatedAssets")
          ) return;
          warn(warning);
        },
      },
    },
  },
  markdown: {
    shikiConfig: {
      themes: { light: "github-light", dark: "github-dark" },
      // Long lines wrap inside the block instead of scrolling sideways.
      wrap: true,
    },
  },
});
