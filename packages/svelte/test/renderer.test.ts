import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import { afterEach, expect, it, vi } from "vitest"

import { createGpuiRenderer } from "../dist/renderer.js"
import { mountGpui } from "../dist/testing.js"
import { fixture } from "./helpers.js"
const roots: { unmount(): void }[] = []
afterEach(() => roots.splice(0).forEach((root) => root.unmount()))
it("runs real runes, derived values, two-way bindings and snippet text", async () => {
  const App = await fixture(
    `<script>import {Column,Text,TextInput,Button} from '@gpui-native/svelte';let count=$state(0);let name=$state('Ada');const doubled=$derived(count*2);</script><Column><Button testId="button" onPress={()=>count++}>Add</Button><TextInput testId="input" bind:value={name}/><Text testId="count">{name}: {doubled}</Text></Column>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  root.findByTestId("button").click()
  root.findByTestId("input").trigger("change", { value: "Grace" })
  await root.flush()
  expect(root.findByTestId("count").text).toBe("Grace: 2")
  expect(root.findByTestId("input").prop("value")).toBe("Grace")
})
it("retains keyed instances, changes branches and releases effects on removal", async () => {
  const cleanup = vi.fn<() => void>()
  const App = await fixture<{ cleanup: () => void }, { reverse: () => void; remove: () => void }>(
    `<script>import {onDestroy} from 'svelte';let {cleanup}=$props();onDestroy(cleanup);let items=$state([1,2,3]);export function reverse(){items.reverse()}export function remove(){items=[]}</script><div testId="rows">{#each items as item (item)}<input testId={'row-'+item} value={String(item)}/>{:else}<text testId="empty">Empty</text>{/each}</div>`,
  )
  const root = mountGpui(App, { cleanup })
  roots.push(root)
  const id = root.findByTestId("row-2").id
  root.instance!.reverse()
  await root.flush()
  expect(root.findByTestId("row-2").id).toBe(id)
  expect(
    root
      .findByTestId("rows")
      .node.children.filter((n) => n.kind === "element")
      .map((n) => (n.kind === "element" ? n.props.testId : "?")),
  ).toEqual(["row-3", "row-2", "row-1"])
  root.instance!.remove()
  await root.flush()
  expect(root.queryByTestId("row-2")).toBeNull()
  expect(root.findByTestId("empty").text).toBe("Empty")
  root.unmount()
  expect(cleanup).toHaveBeenCalledOnce()
  expect(root.renderer.snapshot()).toEqual([])
})
it("touches only the changed row in one atomic mutation wave", async () => {
  const App = await fixture<Record<string, never>, { update: () => void }>(
    `<script>let items=$state(Array.from({length:1000},(_,id)=>({id,value:0})));export function update(){items[500].value++}</script><div>{#each items as item (item.id)}<text>{item.value}</text>{/each}</div>`,
  )
  const renderer = new MemoryNativeRenderer(),
    batches: unknown[][][] = []
  const apply = renderer.applyBatch.bind(renderer)
  renderer.applyBatch = (batch) => {
    batches.push(JSON.parse(batch))
    return apply(batch)
  }
  const host = createGpuiRenderer(renderer)
  roots.push({ unmount: () => host.destroy() })
  const app = host.render(App, {})!
  batches.length = 0
  app.update()
  host.flush()
  expect(batches).toHaveLength(1)
  expect(batches[0]).toHaveLength(1)
  expect(batches[0]![0]![0]).toBe("setText")
})
it("supports async branches without reviving a destroyed root", async () => {
  let resolve!: (value: string) => void
  const promise = new Promise<string>((r) => {
    resolve = r
  })
  const App = await fixture<{ promise: Promise<string> }>(
    `<script>let {promise}=$props()</script>{#await promise}<text testId="pending">Wait</text>{:then value}<text testId="done">{value}</text>{/await}`,
  )
  const root = mountGpui(App, { promise })
  roots.push(root)
  expect(root.findByTestId("pending").text).toBe("Wait")
  resolve("Done")
  await root.flush()
  expect(root.findByTestId("done").text).toBe("Done")
  root.unmount()
  await root.flush()
  expect(root.renderer.snapshot()).toEqual([])
})
