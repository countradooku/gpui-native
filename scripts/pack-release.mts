import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises"
import { resolve } from "node:path"

const root = resolve(import.meta.dir, "..")
const output = resolve(root, "release-assets")
const version = JSON.parse(await readFile(resolve(root, "package.json"), "utf8")).version as string
await mkdir(output, { recursive: true })
for (const target of ["darwin-arm64", "linux-x64-gnu", "win32-x64-msvc"]) {
  const name = `gpui-native.${target}.node`
  await copyFile(resolve(root, "packages/core", name), resolve(output, name))
}
async function pack(name: string) {
  const manifest = JSON.parse(
    await readFile(resolve(root, "packages", name, "package.json"), "utf8"),
  )
  if (manifest.version !== version) throw new Error(`Version mismatch in ${name}`)
  const process = Bun.spawn([Bun.which("bun")!, "pm", "pack", "--destination", output, "--quiet"], {
    cwd: resolve(root, "packages", name),
    stdout: "inherit",
    stderr: "inherit",
  })
  if ((await process.exited) !== 0) throw new Error(`Failed to package ${name}`)
  const archive = resolve(
    output,
    `${manifest.name.replace(/^@/, "").replaceAll("/", "-")}-${version}.tgz`,
  )
  const inspect = Bun.spawn(["tar", "-xOf", archive, "package/package.json"], {
    stdout: "pipe",
    stderr: "inherit",
  })
  const packed = JSON.parse(await new Response(inspect.stdout).text())
  if ((await inspect.exited) !== 0) throw new Error(`Cannot inspect ${archive}`)
  for (const group of [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
  ]) {
    for (const [dependency, range] of Object.entries(packed[group] ?? {})) {
      if (typeof range !== "string" || /^(workspace|catalog):/.test(range))
        throw new Error(`Unresolved dependency ${dependency} in ${archive}`)
      if (dependency.startsWith("@gpui-native/") && range !== `^${version}`)
        throw new Error(`Stale internal dependency ${dependency}@${range} in ${archive}`)
    }
  }
}
for (const name of ["core", "runtime", "vue", "react", "svelte"]) await pack(name)
const vuePath = resolve(root, "packages/vue/package.json")
const original = await readFile(vuePath, "utf8")
try {
  await writeFile(
    vuePath,
    JSON.stringify({ ...JSON.parse(original), name: "gpui-vue" }, null, 2) + "\n",
  )
  await pack("vue")
} finally {
  await writeFile(vuePath, original)
}
