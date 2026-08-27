import { mkdir, rm } from "node:fs/promises"
import { join } from "node:path"

const repositoryRoot = join(import.meta.dir, "..")
const wasmOutput = join(repositoryRoot, "target/wasm32-unknown-unknown/release/gpui_vue_core.wasm")
const bindingsOutput = join(repositoryRoot, "web/pkg")
const viteCli = join(repositoryRoot, "node_modules/vite/bin/vite.js")

async function run(command: string[], environment?: Record<string, string>): Promise<void> {
  const process = Bun.spawn(command, {
    cwd: repositoryRoot,
    env: { ...Bun.env, ...environment },
    stdout: "inherit",
    stderr: "inherit",
  })
  const exitCode = await process.exited
  if (exitCode !== 0) throw new Error(`${command.join(" ")} exited with code ${exitCode}`)
}

await rm(bindingsOutput, { recursive: true, force: true })
await mkdir(bindingsOutput, { recursive: true })

await run(
  [
    "cargo",
    "build",
    "--release",
    "--target",
    "wasm32-unknown-unknown",
    "--package",
    "gpui-vue-core",
    "--no-default-features",
  ],
  { RUSTC_BOOTSTRAP: "1" },
)
await run([
  Bun.env.WASM_BINDGEN ?? "wasm-bindgen",
  wasmOutput,
  "--target",
  "web",
  "--no-typescript",
  "--out-dir",
  bindingsOutput,
  "--out-name",
  "gpui_vue_core",
])
await run([process.execPath, viteCli, "build", "--config", "vite.pages.config.mts"])
