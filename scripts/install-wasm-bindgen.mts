import { join } from "node:path"

const repositoryRoot = join(import.meta.dir, "..")
const manifest = await Bun.file(join(repositoryRoot, "crates/gpui-vue-core/Cargo.toml")).text()
const version = manifest.match(/^wasm-bindgen\s*=\s*"=([^"]+)"/m)?.[1]

if (!version) throw new Error("Could not read the pinned wasm-bindgen version from Cargo.toml")

const installed = Bun.which("wasm-bindgen")
if (installed) {
  const probe = Bun.spawnSync([installed, "--version"], { stdout: "pipe", stderr: "pipe" })
  if (probe.exitCode === 0 && probe.stdout.toString().trim().endsWith(` ${version}`)) {
    console.log(`wasm-bindgen-cli ${version} is already installed`)
    process.exit(0)
  }
}

const install = Bun.spawn(
  ["cargo", "install", "wasm-bindgen-cli", "--version", version, "--locked"],
  {
    cwd: repositoryRoot,
    stdout: "inherit",
    stderr: "inherit",
  },
)
const exitCode = await install.exited
if (exitCode !== 0) throw new Error(`cargo install wasm-bindgen-cli exited with code ${exitCode}`)
