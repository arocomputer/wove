import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://wovetui.com",
  devToolbar: { enabled: false },
  redirects: { "/sidebar": "/" },
  integrations: [mdx(), sitemap()],
  markdown: {
    shikiConfig: { themes: { light: "github-light", dark: "github-dark" } },
  },
});
