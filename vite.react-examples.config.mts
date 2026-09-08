import { defineConfig } from "vite"
export default defineConfig({
  build: {
    target: "node20",
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
    minify: "oxc",
    lib: { entry: "src/main.tsx", formats: ["es"], fileName: "main" },
    rollupOptions: { external: [/^@gpui-native\//, /^react(?:\/|$)/] },
  },
})
