import { MemoryNativeRenderer } from "@gpui-native/runtime/native"
import { afterEach, expect, it, vi } from "vitest"

import { createGpuiRenderer } from "../dist/renderer.js"
import { createGpuiRuntime } from "../dist/runtime-core.js"
import { mountGpui } from "../dist/testing.js"
import { fixture } from "./helpers.js"
const cleanups: (() => void)[] = []
afterEach(() => cleanups.splice(0).forEach((clean) => clean()))
it("updates deep runes styles, removes props and preserves state on root prop updates", async () => {
  const App = await fixture<{ label: string }, { change: () => void }>(
    `<script>let {label}=$props();let style=$state({background:'#fff',padding:5});let count=$state(0);export function change(){style.background='#000';delete style.padding;count++}</script><div testId="view" {style}>{label} {count}</div>`,
  )
  const root = mountGpui(App, { label: "first" })
  cleanups.push(root.unmount)
  const id = root.findByTestId("view").id
  root.instance!.change()
  await root.flush()
  expect(root.findByTestId("view").prop("style")).toEqual({ background: "#000" })
  root.render(App, { label: "second" })
  await root.flush()
  expect(root.findByTestId("view").text).toBe("second 1")
  expect(root.findByTestId("view").id).toBe(id)
})
it("isolates independent windows and renders runtime failures in the owning window", async () => {
  const App = await fixture(
    `<script>import {Button,Text} from '@gpui-native/svelte';let count=$state(0)</script><Button testId="button" onPress={()=>count++}>Add</Button><Text testId="count">{count}</Text>`,
  )
  const runtime = createGpuiRuntime(() => new MemoryNativeRenderer())
  const first = runtime.createWindow(App, { headless: true }),
    second = runtime.createWindow(App, { headless: true })
  cleanups.push(
    () => first.close(),
    () => second.close(),
  )
  expect(first.host.nativeRenderer).not.toBe(second.host.nativeRenderer)
  first.close()
  expect((first.host.nativeRenderer as MemoryNativeRenderer).snapshot()).toEqual([])
  expect((second.host.nativeRenderer as MemoryNativeRenderer).snapshot().length).toBeGreaterThan(0)
})
it("captures Svelte render errors in the native runtime boundary", async () => {
  const error = vi.fn<(error: unknown) => void>()
  const App = await fixture(
    `<script>throw new Error('intentional failure')</script><text>Never</text>`,
  )
  const renderer = new MemoryNativeRenderer(),
    runtime = createGpuiRuntime(() => renderer)
  const root = runtime.createWindow(App, { renderer, onError: error })
  cleanups.push(root.close)
  expect(error).toHaveBeenCalledOnce()
  expect(renderer.snapshot().some((node) => node.text?.includes("intentional failure"))).toBe(true)
})
it("keeps the browser document untouched and supports context, stores and effects", async () => {
  const before = Object.getOwnPropertyDescriptor(globalThis, "document"),
    cleanup = vi.fn<() => void>()
  const App = await fixture<{ cleanup: () => void }, { increment: () => void }>(
    `<script>import {setContext,getContext,onDestroy} from 'svelte';import {writable} from 'svelte/store';let {cleanup}=$props();const count=writable(0);setContext('name','native');onDestroy(cleanup);export function increment(){count.update(n=>n+1)}</script><text testId="text">{getContext('name')}: {$count}</text>`,
  )
  const root = mountGpui(App, { cleanup })
  cleanups.push(root.unmount)
  root.instance!.increment()
  await root.flush()
  expect(root.findByTestId("text").text).toBe("native: 1")
  expect(Object.getOwnPropertyDescriptor(globalThis, "document")).toEqual(before)
  root.unmount()
  expect(cleanup).toHaveBeenCalledOnce()
})
it("keeps GPU canvas identity on resize and destroys the source on unmount", async () => {
  const App = await fixture<Record<string, never>, { resize: () => void; canvas: () => unknown }>(
    `<script>import {useGPUCanvas} from '@gpui-native/svelte';let width=$state(100);const gpu=useGPUCanvas(()=>({width,height:80,presentation:'async-readback'}));export function resize(){width=200}export function canvas(){return gpu.current}</script><text>GPU</text>`,
  )
  const destroyed = vi.fn<(id: number) => void>()
  const native = Object.assign(new MemoryNativeRenderer(), {
    createCanvasSource: () => 1,
    presentCanvasFrame: () => {},
    destroyCanvasSource: destroyed,
  })
  const host = createGpuiRenderer(native)
  cleanups.push(() => host.destroy())
  const app = host.render(App, {})!
  host.flush()
  const canvas = app.canvas() as { id: number; width: number; destroyed: boolean }
  app.resize()
  host.flush()
  expect(app.canvas()).toBe(canvas)
  expect(canvas.width).toBe(200)
  host.destroy()
  expect(canvas.destroyed).toBe(true)
  expect(destroyed).toHaveBeenCalledOnce()
})
it("merges component window-key subscriptions with root callbacks and cleans both", async () => {
  const { handleGpuiEvent } = await import("@gpui-native/runtime/events")
  const hook = vi.fn<() => void>(),
    top = vi.fn<() => void>()
  const App = await fixture<{ hook: () => void }>(
    `<script>import {useWindowEvent} from '@gpui-native/svelte';let {hook}=$props();useWindowEvent('windowKeyDown',hook)</script><text>Keys</text>`,
  )
  let id = 0,
    enabled = false
  const native = Object.assign(new MemoryNativeRenderer(), {
    setWindowKeyEvents: (down: boolean, _up: boolean, next: number) => {
      id = next
      enabled = down
    },
  })
  const runtime = createGpuiRuntime(() => native)
  const root = runtime.createWindow(App, { renderer: native, props: { hook }, onKeyDown: top })
  cleanups.push(root.close)
  expect(enabled).toBe(true)
  handleGpuiEvent({ elementId: id, eventType: "windowKeyDown", key: "a" }, native)
  expect(hook).toHaveBeenCalledOnce()
  expect(top).toHaveBeenCalledOnce()
  root.close()
  expect(enabled).toBe(false)
  expect(handleGpuiEvent({ elementId: id, eventType: "windowKeyDown", key: "a" }, native)).toBe(
    false,
  )
})
it("reads window geometry after an asynchronous Web window opens", async () => {
  const { handleGpuiEvent } = await import("@gpui-native/runtime/events")
  let ready = false
  const native = Object.assign(new MemoryNativeRenderer(), {
    supportsWindowEvents: () => true,
    getWindowSize: () => {
      if (!ready) throw new Error("Window is opening")
      return { width: 1024, height: 768 }
    },
    getWindowInsets: () => {
      if (!ready) throw new Error("Window is opening")
      const zero = { top: 0, left: 0, right: 0, bottom: 0 }
      return { safeArea: zero, ime: { ...zero, bottom: 100 }, effective: { ...zero, bottom: 100 } }
    },
  })
  const App = await fixture(
    `<script>import {useWindowSize,useWindowInsets} from '@gpui-native/svelte';const size=useWindowSize(),insets=useWindowInsets()</script><text testId="geometry">{size.width}/{insets.visibleHeight}</text>`,
  )
  const host = createGpuiRenderer(native)
  cleanups.push(host.destroy)
  host.render(App)
  ready = true
  handleGpuiEvent({ elementId: 0, eventType: "windowResize" }, native)
  host.flush()
  expect(native.snapshot().some((node) => node.text === "1024/668")).toBe(true)
  host.destroy()
  expect(handleGpuiEvent({ elementId: 0, eventType: "windowResize" }, native)).toBe(false)
})
it("does not resend unchanged structured props when another rune changes", async () => {
  const App = await fixture<Record<string, never>, { update: () => void }>(
    `<script>import {Canvas,useTextSearch,View} from '@gpui-native/svelte';let opacity=$state(1);const commands=[{type:'rect',x:0,y:0,width:10,height:10,fill:'#fff'}];const search=useTextSearch(()=>({query:'native'}));export function update(){opacity=.5}</script><View {...search.props}><Canvas {commands} style={{opacity}}/></View>`,
  )
  const native = new MemoryNativeRenderer(),
    host = createGpuiRenderer(native)
  cleanups.push(host.destroy)
  const app = host.render(App)!
  const batches: unknown[][][] = []
  const apply = native.applyBatch.bind(native)
  native.applyBatch = (batch) => {
    batches.push(JSON.parse(batch))
    return apply(batch)
  }
  app.update()
  host.flush()
  expect(batches.flat().map((operation) => operation[0])).toEqual(["setStyle"])
})
