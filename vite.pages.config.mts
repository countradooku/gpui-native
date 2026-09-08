import { resolve } from "node:path"

import vue from "@vitejs/plugin-vue"
import { defineConfig } from "vite"

const repositoryRoot = import.meta.dirname
const webRoot = resolve(repositoryRoot, "web")
const pagesBase = process.env.PAGES_BASE_PATH ?? "/"

export default defineConfig({
  root: webRoot,
  base: pagesBase,
  plugins: [vue()],
  resolve: {
    alias: [
      {
        find: /^@gpui-native\/react$/,
        replacement: resolve(repositoryRoot, "packages/react/src/web.ts"),
      },
      {
        find: /^(?:@gpui-native\/vue|gpui-vue)$/,
        replacement: resolve(repositoryRoot, "packages/vue/src/web.ts"),
      },
      {
        find: "@gpui-native/wasm",
        replacement: resolve(webRoot, "pkg/gpui_core.js"),
      },
    ],
  },
  build: {
    target: "es2022",
    outDir: resolve(repositoryRoot, "dist-pages"),
    emptyOutDir: true,
    sourcemap: true,
    minify: "oxc",
    rollupOptions: {
      input: {
        index: resolve(webRoot, "index.html"),
        "react-counter": resolve(webRoot, "examples/react-counter/index.html"),
        "react-showcase": resolve(webRoot, "examples/react-showcase/index.html"),
        "react-multiple-windows": resolve(webRoot, "examples/react-multiple-windows/index.html"),
        counter: resolve(webRoot, "examples/counter/index.html"),
        canvas: resolve(webRoot, "examples/canvas/index.html"),
        "market-stream": resolve(webRoot, "examples/market-stream/index.html"),
        "motion-timeline": resolve(webRoot, "examples/motion-timeline/index.html"),
        "multiple-windows": resolve(webRoot, "examples/multiple-windows/index.html"),
        "audio-buffer": resolve(webRoot, "examples/audio-buffer/index.html"),
      },
    },
  },
})
