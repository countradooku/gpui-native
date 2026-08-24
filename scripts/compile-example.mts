import { join } from "node:path"

interface ExamplePackage {
  name: string
}

const exampleRoot = process.cwd()
const packageJson = (await Bun.file(join(exampleRoot, "package.json")).json()) as ExamplePackage
const exampleName = packageJson.name.replace(/^@gpui-vue\/example-/, "")
const executableName = `gpui-vue-${exampleName}${process.platform === "win32" ? ".exe" : ""}`
const entrypoint = join(exampleRoot, "dist/main.js")
const outfile = join(exampleRoot, `dist/${executableName}`)

const result = await Bun.build({
  entrypoints: [entrypoint],
  compile: {
    outfile,
    autoloadBunfig: false,
    autoloadDotenv: false,
  },
  bytecode: true,
  minify: true,
  sourcemap: "linked",
})

if (!result.success) {
  for (const log of result.logs) console.error(log)
  process.exit(1)
}

console.log(`[bun] compiled ${packageJson.name} -> ${outfile}`)
