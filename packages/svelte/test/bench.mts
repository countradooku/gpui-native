import assert from "node:assert/strict"

import { MemoryNativeRenderer } from "@gpui-native/runtime/native"

import { createGpuiRenderer } from "../dist/renderer.js"
import { fixture } from "./helpers.js"
const rows = 10000,
  iterations = 1000
const App = await fixture<Record<string, never>, { update: () => void }>(
  `<script>let rows=$state(Array.from({length:${rows}},(_,id)=>({id,value:0})));export function update(){rows[5000].value++}</script><div>{#each rows as row (row.id)}<text>{row.value}</text>{/each}</div>`,
)
const native = new MemoryNativeRenderer(),
  host = createGpuiRenderer(native)
let batches = 0,
  mutations = 0
const apply = native.applyBatch.bind(native)
native.applyBatch = (batch) => {
  batches++
  mutations += JSON.parse(batch).length
  return apply(batch)
}
const mountStart = performance.now()
const app = host.render(App, {})!
const mountMs = performance.now() - mountStart
for (let i = 0; i < 20; i++) {
  app.update()
  host.flush()
}
batches = mutations = 0
const timings: number[] = []
for (let i = 0; i < iterations; i++) {
  const start = performance.now()
  app.update()
  host.flush()
  timings.push(performance.now() - start)
}
assert.equal(batches, iterations)
assert.equal(mutations, iterations)
timings.sort((a, b) => a - b)
console.log(
  JSON.stringify(
    {
      renderer:
        "Svelte runes → MemoryNativeRenderer (includes JSON batch decode and transactional tree copy)",
      rows,
      iterations,
      mountMs,
      p50Ms: timings[Math.floor(iterations * 0.5)],
      p95Ms: timings[Math.floor(iterations * 0.95)],
      batchesPerUpdate: batches / iterations,
      mutationsPerUpdate: mutations / iterations,
    },
    null,
    2,
  ),
)
// A discard sink isolates scheduling, host diffing and JSON encoding from the
// memory renderer's deliberately conservative full-tree transaction copy.
const adapterTimings: number[] = []
native.applyBatch = (batch) => {
  const ops = JSON.parse(batch)
  assert.equal(ops.length, 1)
  assert.equal(ops[0][0], "setText")
}
for (let i = 0; i < iterations; i++) {
  const start = performance.now()
  app.update()
  host.flush()
  adapterTimings.push(performance.now() - start)
}
adapterTimings.sort((a, b) => a - b)
console.log(
  JSON.stringify(
    {
      renderer:
        "Adapter only → discard sink (includes JSON encode/decode; excludes retained backend/layout/paint)",
      rows,
      iterations,
      p50Ms: adapterTimings[Math.floor(iterations * 0.5)],
      p95Ms: adapterTimings[Math.floor(iterations * 0.95)],
    },
    null,
    2,
  ),
)
native.applyBatch = apply
host.destroy()
assert.equal(native.snapshot().length, 0)
