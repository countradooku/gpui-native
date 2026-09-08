import { readdir, readFile, mkdir, rm, writeFile } from "node:fs/promises"
import { basename, dirname, join, relative } from "node:path"

import { transformSync } from "oxc-transform"

const repositoryRoot = join(import.meta.dir, "..")
const packageRoot = join(repositoryRoot, "packages/vue")
const sourceRoot = join(packageRoot, "src")
const outputRoot = join(packageRoot, "dist")
const tsconfigPath = join(packageRoot, "tsconfig.build.json")
const tscPath = join(packageRoot, "node_modules/typescript/bin/tsc")

async function collectTypeScriptFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = await Promise.all(
    entries.map(async (entry) => {
      const path = join(directory, entry.name)

      if (entry.isDirectory()) return collectTypeScriptFiles(path)
      if (entry.isFile() && entry.name.endsWith(".ts") && !entry.name.endsWith(".d.ts")) {
        return [path]
      }

      return []
    }),
  )

  return files.flat().sort()
}

async function emitDeclarations(): Promise<void> {
  const declarationProcess = Bun.spawn([process.execPath, tscPath, "-p", tsconfigPath], {
    cwd: packageRoot,
    stdout: "inherit",
    stderr: "inherit",
  })
  const exitCode = await declarationProcess.exited

  if (exitCode !== 0) {
    throw new Error(`TypeScript declaration emit failed with exit code ${exitCode}`)
  }
}

function formatTransformErrors(
  filename: string,
  errors: ReturnType<typeof transformSync>["errors"],
): string {
  return errors
    .map((error) => {
      const frame = error.codeframe ? `\n${error.codeframe}` : ""
      return `${filename}: ${error.severity}: ${error.message}${frame}`
    })
    .join("\n")
}

async function transformModule(sourcePath: string): Promise<void> {
  const relativeSourcePath = relative(packageRoot, sourcePath)
  const relativeOutputPath = relative(sourceRoot, sourcePath).replace(/\.ts$/, ".js")
  const outputPath = join(outputRoot, relativeOutputPath)
  const source = await readFile(sourcePath, "utf8")
  const result = transformSync(relativeSourcePath, source, {
    cwd: packageRoot,
    lang: "ts",
    sourceType: "module",
    target: "es2022",
    sourcemap: true,
    typescript: {
      onlyRemoveTypeImports: true,
    },
  })

  if (result.errors.length > 0) {
    throw new Error(formatTransformErrors(relativeSourcePath, result.errors))
  }

  await mkdir(dirname(outputPath), { recursive: true })

  const sourceMapFilename = `${basename(outputPath)}.map`
  const output = `${result.code.trimEnd()}\n//# sourceMappingURL=${sourceMapFilename}\n`
  await writeFile(outputPath, output)

  if (result.map) {
    result.map.file = basename(outputPath)
    result.map.sources = result.map.sources.map(() =>
      relative(dirname(outputPath), sourcePath).replaceAll("\\", "/"),
    )
    await writeFile(`${outputPath}.map`, `${JSON.stringify(result.map)}\n`)
  }
}

await rm(outputRoot, { recursive: true, force: true })
await emitDeclarations()

const sources = await collectTypeScriptFiles(sourceRoot)
await Promise.all(sources.map(transformModule))

console.log(`[oxc] transformed ${sources.length} TypeScript modules`)
