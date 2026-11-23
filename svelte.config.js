// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import { readFileSync } from "node:fs";

// ビルドの版の名前を、package.json の version に固定する。
// 既定では設定を読むたびに時刻から作られるため、ビルド中に設定が何度も読まれる環境
// （Node 24 で確認）では index.html と画面の部品とで名前が食い違い、画面が真っ白になる
const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    version: { name: pkg.version },
    adapter: adapter({
      fallback: "index.html",
    }),
  },
};

export default config;
