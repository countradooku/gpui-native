import { expect, it } from "vitest"

import { compileNative } from "../dist/compiler.js"
import { mountGpui } from "../dist/testing.js"
import { fixture } from "./helpers.js"
it("compiles nested raw hosts without renaming application strings or identifiers", async () => {
  const App = await fixture(
    `<script>const GpuiNativeHostInternal='svelte'</script><div><div testId="inner">{GpuiNativeHostInternal}</div></div>`,
  )
  const root = mountGpui(App)
  try {
    expect(root.findByTestId("inner").text).toBe("svelte")
  } finally {
    root.unmount()
  }
})
it("uses tree templates and rejects unsupported browser features clearly", () => {
  const result = compileNative("<text>Hello</text>", "Hello.svelte")
  expect(result.js.code).not.toContain("from_html")
  expect(result.js.code).toContain("@gpui-native/svelte/engine")
  for (const source of [
    "<button>DOM</button>",
    "<style>text{color:red}</style><text>Hello</text>",
    "<svelte:window/>",
    '{@html "<div/>"}',
  ])
    expect(() => compileNative(source)).toThrow(/GPUI|native|browser-only/)
})
it("compiles TypeScript rune modules with the same isolated runtime", () => {
  const result = compileNative(
    "let value=$state<number>(0);export function increment():number{return ++value}",
    "counter.svelte.ts",
  )
  expect(result.js.code).toContain("@gpui-native/svelte/engine")
  expect(result.js.code).not.toContain("$state<number>")
})
it("rewrites real static and dynamic Svelte imports without changing string literals", async () => {
  const { rewriteImports } = await import("../dist/compiler.js")
  const source = `const example = 'import "svelte"'; export { onMount } from 'svelte'; const stores = import('svelte/store')`
  const output = rewriteImports(source)
  expect(output).toContain(`'import "svelte"'`)
  expect(output).toContain('from "@gpui-native/svelte/public"')
  expect(output).toContain('import("@gpui-native/svelte/store")')
})
it("accepts Svelte lowercase event callbacks alongside the shared native names", async () => {
  const App = await fixture(
    `<script>import {Button} from '@gpui-native/svelte';let count=$state(0)</script><Button testId="button" onpress={()=>count++}>{count}</Button><div testId="raw" onclick={()=>count++}/>`,
  )
  const root = mountGpui(App)
  try {
    root.findByTestId("button").click()
    root.findByTestId("raw").click()
    await root.flush()
    expect(root.findByTestId("button").text).toBe("2")
  } finally {
    root.unmount()
  }
})
