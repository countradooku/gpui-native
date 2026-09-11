import { mkdir, readFile, writeFile, readdir, copyFile } from "node:fs/promises"
import { createRequire } from "node:module"
import { dirname, join, resolve } from "node:path"

const packageRoot = resolve(import.meta.dirname, "../packages/svelte")
const require = createRequire(join(packageRoot, "package.json"))
const { build } = await import(require.resolve("esbuild"))
const ts = await import(require.resolve("typescript"))
const { compileModule } = await import(require.resolve("svelte/compiler"))
const svelteRoot = dirname(require.resolve("svelte/package.json"))
const out = join(packageRoot, "dist")
await mkdir(out, { recursive: true })
await build({
  stdin: {
    contents: `export * from 'svelte/internal/client';
      export * as api from 'svelte';
      export * as stores from 'svelte/store';
      export * as reactivity from 'svelte/reactivity';
      export { assign_nodes } from ${JSON.stringify(join(svelteRoot, "src/internal/client/dom/template.js"))};`,
    resolveDir: packageRoot,
  },
  outfile: join(out, "engine.js"),
  bundle: true,
  format: "esm",
  platform: "neutral",
  conditions: ["browser", "production"],
  target: "es2022",
  sourcemap: true,
  inject: [join(packageRoot, "src/runtime-globals.ts")],
  define: { "globalThis.document": "document", "globalThis.window": "window" },
  plugins: [
    {
      name: "native-host",
      setup(builder: import("esbuild").PluginBuild) {
        builder.onResolve({ filter: /^\.\/host\.js$/ }, () => ({
          path: "./host.js",
          external: true,
        }))
      },
    },
  ],
})
await copyFile(join(packageRoot, "src/engine.d.ts"), join(out, "engine.d.ts"))
const process = Bun.spawn(
  [Bun.which("bun")!, require.resolve("typescript/bin/tsc"), "-p", "tsconfig.build.json"],
  { cwd: packageRoot, stdout: "inherit", stderr: "inherit" },
)
if ((await process.exited) !== 0) throw new Error("Svelte adapter type declaration build failed")
for (const file of await readdir(join(packageRoot, "src"))) {
  if (!file.endsWith(".svelte.ts")) continue
  const source = await readFile(join(packageRoot, "src", file), "utf8")
  const javascript = ts.transpileModule(source, {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.ESNext,
      verbatimModuleSyntax: true,
    },
  }).outputText
  const result = compileModule(javascript, { filename: file, generate: "client", dev: false })
  await writeFile(
    join(out, file.replace(/\.ts$/, ".js")),
    result.js.code
      .replaceAll('"svelte/internal/client"', '"./engine.js"')
      .replaceAll("'svelte/internal/client'", '"./engine.js"'),
  )
}

const { compileNative } = await import(join(out, "compiler.js"))
await mkdir(join(out, "controls"), { recursive: true })
for (const file of await readdir(join(packageRoot, "src/controls"))) {
  if (!file.endsWith(".svelte")) continue
  const result = compileNative(
    await readFile(join(packageRoot, "src/controls", file), "utf8"),
    file,
  )
  await writeFile(
    join(out, "controls", file + ".js"),
    result.js.code.replace(
      /(["'])([^"']+\.svelte)\1/g,
      (_: string, q: string, p: string) => `${q}${p}.js${q}`,
    ),
  )
}
const { emitDts } = await import(require.resolve("svelte2tsx"))
await emitDts({
  libRoot: join(packageRoot, "src"),
  declarationDir: out,
  tsconfig: join(packageRoot, "tsconfig.build.json"),
  svelteShimsPath: require.resolve("svelte2tsx/svelte-shims-v4.d.ts"),
})
for (const file of ["controls.js", "controls.d.ts", "runtime-core.js"]) {
  const path = join(out, file)
  await writeFile(
    path,
    (await readFile(path, "utf8")).replace(
      /(["'])([^"']+\.svelte)\1/g,
      (_: string, q: string, p: string) => `${q}${p}.js${q}`,
    ),
  )
}

console.log("Svelte adapter and isolated Svelte runtime built")
