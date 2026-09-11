import { afterEach, expect, it, vi } from "vitest"

import { mountGpui } from "../dist/testing.js"
import { fixture } from "./helpers.js"
const roots: { unmount(): void }[] = []
afterEach(() => {
  roots.splice(0).forEach((root) => root.unmount())
  vi.useRealTimers()
})
it("selects using bound state and keyboard and keeps labels registered while closed", async () => {
  const App = await fixture(
    `<script>import {Select,SelectTrigger,SelectValue,SelectContent,SelectItem,Text} from '@gpui-native/svelte';let value=$state('a')</script><Select bind:value><SelectTrigger testId="trigger"><SelectValue testId="label"/></SelectTrigger><SelectContent testId="content"><SelectItem value="a" textValue="Alpha">Alpha</SelectItem><SelectItem value="b" textValue="Beta" testId="beta">Beta</SelectItem><SelectItem value="c" disabled>Disabled</SelectItem></SelectContent></Select><Text testId="value">{value}</Text>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  await root.flush()
  expect(root.findByTestId("label").text).toBe("Alpha")
  root.findByTestId("trigger").click()
  await root.flush()
  root.findByTestId("beta").click()
  await root.flush()
  expect(root.findByTestId("value").text).toBe("b")
  expect(root.findByTestId("trigger").prop("aria-expanded")).toBe(false)
  root.findByTestId("trigger").trigger("keyDown", { key: "a" })
  await root.flush()
  expect(root.findByTestId("value").text).toBe("a")
})
it("filters combobox items and binds selection", async () => {
  const App = await fixture(
    `<script>import {Combobox,ComboboxInput,ComboboxContent,ComboboxItem,ComboboxEmpty,Text} from '@gpui-native/svelte';let value=$state('')</script><Combobox bind:value><ComboboxInput testId="input"/><ComboboxContent><ComboboxItem value="a" textValue="Apple" testId="apple">Apple</ComboboxItem><ComboboxItem value="b" textValue="Banana" testId="banana">Banana</ComboboxItem><ComboboxEmpty testId="empty">Nothing</ComboboxEmpty></ComboboxContent></Combobox><Text testId="value">{value}</Text>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  await root.flush()
  root.findByTestId("input").trigger("change", { value: "Ban" })
  await root.flush()
  expect(root.findByTestId("apple").prop("style")).toMatchObject({ display: "none" })
  root.findByTestId("input").trigger("keyDown", { key: "enter" })
  await root.flush()
  expect(root.findByTestId("value").text).toBe("b")
  root.findByTestId("input").trigger("change", { value: "zzz" })
  await root.flush()
  expect(root.findByTestId("empty").text).toBe("Nothing")
})
it("opens tooltips on focus and cancels pending timers on removal", async () => {
  const App = await fixture(
    `<script>import {Tooltip,TooltipTrigger,TooltipContent} from '@gpui-native/svelte'</script><Tooltip delayDuration={100}><TooltipTrigger testId="trigger">Hover</TooltipTrigger><TooltipContent testId="content">Tip</TooltipContent></Tooltip>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  root.findByTestId("trigger").trigger("focus")
  await root.flush()
  expect(root.findByTestId("content").text).toBe("Tip")
  root.findByTestId("trigger").trigger("blur")
  await root.flush()
  expect(root.queryByTestId("content")).toBeNull()
  vi.useFakeTimers()
  root.findByTestId("trigger").trigger("mouseEnter")
  root.unmount()
  vi.advanceTimersByTime(150)
  await root.flush()
  expect(root.renderer.snapshot()).toEqual([])
})
it("opens upwards at the last enabled item and exposes item state to styles and snippets", async () => {
  const App = await fixture(
    `<script>import {Select,SelectTrigger,SelectContent,SelectItem,Text} from '@gpui-native/svelte';let value=$state('')</script><Select bind:value><SelectTrigger testId="trigger">Select</SelectTrigger><SelectContent><SelectItem value="a">A</SelectItem><SelectItem value="b" testId="b" style={item=>({opacity:item.highlighted?1:0.5})}>{#snippet content(item)}<Text>{item.highlighted?'active':'idle'}</Text>{/snippet}</SelectItem><SelectItem value="c" disabled>C</SelectItem></SelectContent></Select><Text testId="value">{value}</Text>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  root.findByTestId("trigger").trigger("keyDown", { key: "up" })
  await root.flush()
  expect(root.findByTestId("b").prop("style")).toMatchObject({ opacity: 1 })
  expect(root.findByTestId("b").text).toBe("active")
  root.findByTestId("trigger").trigger("keyDown", { key: "enter" })
  await root.flush()
  expect(root.findByTestId("value").text).toBe("b")
  expect(root.findByTestId("b").prop("tabIndex")).toBe(-1)
})
it("renders supplied combobox items through a snippet and supports multiple selection", async () => {
  const App = await fixture(
    `<script>import {Combobox,ComboboxInput,ComboboxContent,ComboboxList,ComboboxItem,Text} from '@gpui-native/svelte';let value=$state([])</script><Combobox items={['a','b','c']} itemToStringValue={v=>v.toUpperCase()} bind:value multiple defaultOpen><ComboboxInput testId="input"/><ComboboxContent><ComboboxList>{#snippet item(value)}<ComboboxItem {value} testId={value}>{value}</ComboboxItem>{/snippet}</ComboboxList></ComboboxContent></Combobox><Text testId="value">{value.join(',')}</Text>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  root.findByTestId("input").trigger("change", { value: "b" })
  await root.flush()
  expect(root.queryByTestId("a")).toBeNull()
  root.findByTestId("b").click()
  await root.flush()
  expect(root.findByTestId("value").text).toBe("b")
  expect(root.findByTestId("input").prop("value")).toBe("")
  root.findByTestId("a").click()
  await root.flush()
  expect(root.findByTestId("value").text).toBe("b,a")
})
it("composes a trigger snippet with a bound native ref", async () => {
  const App = await fixture(
    `<script>import {Select,SelectTrigger,SelectContent,SelectItem,View} from '@gpui-native/svelte'</script><Select><SelectTrigger>{#snippet child(host)}<View {...host} bind:ref={host.ref} testId="custom">Open</View>{/snippet}</SelectTrigger><SelectContent><SelectItem value="a" testId="item">A</SelectItem></SelectContent></Select>`,
  )
  const root = mountGpui(App)
  roots.push(root)
  root.findByTestId("custom").click()
  await root.flush()
  expect(root.findByTestId("custom").prop("aria-expanded")).toBe(true)
  root.findByTestId("item").click()
  await root.flush()
  expect(root.findByTestId("custom").prop("aria-expanded")).toBe(false)
})
