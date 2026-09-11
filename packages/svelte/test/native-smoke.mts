import assert from "node:assert/strict"

import { createTestRoot, hasNativeTestRenderer } from "../dist/testing.js"
import { fixture } from "./helpers.js"
if (!hasNativeTestRenderer) {
  console.log("Svelte GPU smoke skipped: native test renderer unavailable")
  process.exit(0)
}
const App = await fixture(
  `<script>import {Column,Button,Text,TextInput,Code,Markdown,Canvas} from '@gpui-native/svelte';let count=$state(0),value=$state('hello')</script><Column style={{width:640,height:480,background:'#fff',color:'#111'}}><Button testId="button" aria-label="Svelte counter" disabled={count>=3} onPress={()=>count++} style={{width:200,height:40}}>Count {count}</Button><TextInput testId="input" bind:value style={{width:300,height:50}}/><Code code="let count = $state(0)" language="typescript" style={{height:60}}/><Markdown source="# Svelte native" style={{height:70}}/><Canvas commands={[{type:'rect',x:0,y:0,width:80,height:30,fill:'#f00'}]} style={{height:50}}/></Column>`,
)
const root = createTestRoot({ width: 640, height: 480 })
try {
  root.render(App)
  const button = root.findByTestId("button"),
    bounds = root.renderer.getElementBounds(button.id)
  assert(bounds && bounds[2]! > 0)
  root.renderer.nativeSimulateClick(bounds[0]! + 5, bounds[1]! + 5)
  await root.flush()
  assert.equal(root.findByTestId("button").text, "Count 1")
  root.renderer.nativeSimulateKeyDown(button.id, "enter")
  await root.flush()
  root.renderer.nativeSimulateKeyDown(button.id, "space")
  await root.flush()
  assert.equal(root.findByTestId("button").text, "Count 2")
  root.renderer.nativeSimulateKeyUp(button.id, "space")
  await root.flush()
  assert.equal(root.findByTestId("button").text, "Count 3")
  root.renderer.nativeSimulateClick(bounds[0]! + 5, bounds[1]! + 5)
  await root.flush()
  assert.equal(root.findByTestId("button").text, "Count 3")
  assert(root.renderer.getPaintedText().includes("Count 3"))
  assert(JSON.stringify(root.renderer.getA11yTree()).includes("Svelte counter"))
  const input = root.findByTestId("input")
  root.renderer.nativeSimulateKeystrokes(input.id, "end x")
  await root.flush()
  assert.equal(root.findByTestId("input").prop("value"), "hellox")
} finally {
  root.unmount()
}
assert.equal(root.renderer.getRetainedElementCount(), 0)
console.log(
  "Svelte GPU layout, pointer/keyboard button, bound native input, text, accessibility and cleanup passed",
)
const Controls = await fixture(
  `<script>import {Column,Select,SelectTrigger,SelectContent,SelectItem,Text} from '@gpui-native/svelte';let value=$state('')</script><Column style={{width:640,height:480}}><Select bind:value><SelectTrigger testId="trigger" style={{width:180,height:40}}>Select</SelectTrigger><SelectContent style={{width:180,height:100,background:'#fff'}}><SelectItem value="a" testId="alpha" style={{height:40}}>Alpha</SelectItem><SelectItem value="b" testId="beta" style={{height:40}}>Beta</SelectItem></SelectContent></Select><Text testId="selected">{value}</Text></Column>`,
)
const controls = createTestRoot({ width: 640, height: 480 })
try {
  controls.render(Controls)
  const trigger = controls.findByTestId("trigger")
  controls.renderer.nativeSimulateKeyDown(trigger.id, "up")
  await controls.flush()
  assert.equal(controls.findByTestId("trigger").prop("aria-expanded"), true)
  controls.renderer.nativeSimulateKeyDown(trigger.id, "enter")
  await controls.flush()
  assert.equal(controls.findByTestId("selected").text, "b")
  assert.equal(controls.findByTestId("trigger").prop("aria-expanded"), false)
  const bounds = controls.renderer.getElementBounds(trigger.id)!
  controls.renderer.nativeSimulateClick(bounds[0]! + 5, bounds[1]! + 5)
  await controls.flush()
  const itemBounds = controls.renderer.getElementBounds(controls.findByTestId("alpha").id)!
  assert(itemBounds[2]! > 0 && itemBounds[3]! > 0)
  controls.renderer.nativeSimulateMouseMove(itemBounds[0]! + 5, itemBounds[1]! + 5)
  await controls.flush()
  controls.renderer.nativeSimulateClick(itemBounds[0]! + 5, itemBounds[1]! + 5)
  await controls.flush()
  assert.equal(controls.findByTestId("selected").text, "a")
} finally {
  controls.unmount()
}
assert.equal(controls.renderer.getRetainedElementCount(), 0)
console.log(
  "Svelte native Select keyboard navigation, floating content and pointer selection passed",
)
const Search = await fixture<Record<string, never>, { total: () => number; next: () => void }>(
  `<script>import {View,Text,useTextSearch} from '@gpui-native/svelte';const search=useTextSearch(()=>({query:'native'}));export function total(){return search.total}export function next(){search.next()}</script><View {...search.props}><Text>Native text and native highlighting</Text></View>`,
)
const searchRoot = createTestRoot()
try {
  const search = searchRoot.render(Search)!
  await searchRoot.flush()
  assert.equal(search.total(), 2)
  search.next()
  await searchRoot.flush()
  assert.equal(searchRoot.renderer.getPaintedHighlights().filter((match) => match.active).length, 1)
  assert.equal(searchRoot.renderer.getPaintedHighlights().find((match) => match.active)?.start, 16)
} finally {
  searchRoot.unmount()
}
console.log("Svelte native search counts and active-match navigation passed")
