import { fileURLToPath } from "node:url"

import type { Plugin } from "vite"

import { compileNative, nativeRuntimeImport } from "./compiler.js"

const adapterDirectory = fileURLToPath(new URL(".", import.meta.url)).replaceAll("\\", "/")

/** Compile native components and rune modules with the pinned Svelte compiler. */
export function gpuiSvelte(): Plugin {
  return {
    name: "gpui-svelte",
    enforce: "pre",
    config() {
      return { optimizeDeps: { exclude: ["svelte", "@gpui-native/svelte"] } }
    },
    async resolveId(source, importer) {
      const mapped = nativeRuntimeImport(source)
      if (mapped) return this.resolve(mapped, importer, { skipSelf: true })
      return null
    },
    transform(source, id) {
      const filename = id.split("?")[0]!.replaceAll("\\", "/")
      // Published controls and rune hooks are already compiled with this runtime.
      if (filename.startsWith(adapterDirectory)) return null
      if (!/\.svelte(?:\.[jt]s)?$/.test(filename)) return null
      const result = compileNative(source, filename)
      for (const warning of result.warnings)
        this.warn({
          message: warning.message,
          ...(warning.start
            ? { loc: { file: filename, line: warning.start.line, column: warning.start.column } }
            : {}),
        })
      return { code: result.js.code, map: result.js.map }
    },
    handleHotUpdate(context) {
      if (/\.svelte(?:\.[jt]s)?$/.test(context.file)) {
        context.server.ws.send({ type: "full-reload" })
        return []
      }
    },
  }
}
