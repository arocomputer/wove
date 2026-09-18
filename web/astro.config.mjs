import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";

// Set SITE_URL to the production origin when choosing a host.
export default defineConfig({
  site: process.env.SITE_URL,
  integrations: [mdx(), ...(process.env.SITE_URL ? [sitemap()] : [])],
  markdown: {
    shikiConfig: { themes: { light: "github-light", dark: "github-dark" } },
  },
});
