import { writeFile, mkdir } from "node:fs/promises"
import { join } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"

import type { Component } from "svelte"

import { compileNative } from "../dist/compiler.js"
let sequence = 0
export async function fixture<
  Props extends Record<string, unknown> = Record<string, unknown>,
  Exports extends Record<string, unknown> = Record<string, unknown>,
>(source: string): Promise<Component<Props, Exports>> {
  const folder = fileURLToPath(new URL("./.compiled/", import.meta.url))
  await mkdir(folder, { recursive: true })
  const path = join(folder, `fixture-${process.pid}-${Date.now()}-${sequence++}.js`)
  await writeFile(path, compileNative(source, path.replace(/\.js$/, ".svelte")).js.code)
  return (await import(pathToFileURL(path).href)).default
}
