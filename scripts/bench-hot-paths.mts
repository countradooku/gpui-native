import {
  createAudioFrames,
  createGpuiRenderer,
  createNodeOps,
  h,
  MemoryNativeRenderer,
  wrapWithBatching,
  type NativeRenderer,
} from "../packages/vue/src/index.ts"

interface Measurement {
  name: string
  medianMs: number
  operations: number
  unit: string
}

let sink = 0

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.floor(sorted.length / 2)] ?? 0
}

function measure(
  name: string,
  operations: number,
  unit: string,
  run: () => void,
  rounds = 7,
): Measurement {
  run()
  run()
  const samples: number[] = []
  for (let round = 0; round < rounds; round += 1) {
    const started = performance.now()
    run()
    samples.push(performance.now() - started)
  }
  return { name, medianMs: median(samples), operations, unit }
}

function rate(measurement: Measurement): number {
  return measurement.operations / (measurement.medianMs / 1_000)
}

function requireAtLeast(actual: number, expected: number, label: string): void {
  if (actual < expected) {
    throw new Error(
      `${label}: expected at least ${expected.toLocaleString()}, got ${actual.toFixed(0)}`,
    )
  }
}

function countingRenderer() {
  const counts = new Map<string, number>()
  const count = (operation: string): void => {
    counts.set(operation, (counts.get(operation) ?? 0) + 1)
  }
  const renderer: NativeRenderer = {
    createElement: () => count("createElement"),
    destroyElement: () => {
      count("destroyElement")
      return []
    },
    appendChild: () => count("appendChild"),
    removeChild: () => count("removeChild"),
    insertBefore: () => count("insertBefore"),
    setStyle: () => count("setStyle"),
    setText: () => count("setText"),
    setEventListener: () => count("setEventListener"),
    setRoot: () => count("setRoot"),
    setCustomProp: () => count("setCustomProp"),
    commitMutations: () => count("commitMutations"),
  }
  return { counts, renderer }
}

const boundary = countingRenderer()
const textHost = createGpuiRenderer(boundary.renderer)
textHost.render(h("div", null, "before"))
boundary.counts.clear()
textHost.render(h("div", null, "after"))
const textMutationCount = [...boundary.counts.values()].reduce((total, count) => total + count, 0)
if (boundary.counts.get("setText") !== 1 || textMutationCount !== 2) {
  // The fallback protocol commits once after its one mutation.
  throw new Error(`Text patch regression: ${JSON.stringify(Object.fromEntries(boundary.counts))}`)
}

const siblingCount = 12_000
const siblingQueries = 24_000
const siblingRenderer = new MemoryNativeRenderer()
let nextId = 0
const nodeOps = createNodeOps(siblingRenderer, () => ++nextId)
const parent = nodeOps.createElement("div")
const siblings = Array.from({ length: siblingCount }, (_, index) => {
  const node = nodeOps.createText(String(index))
  nodeOps.insert(node, parent)
  return node
})
const linkedSiblings = measure("linked nextSibling", siblingQueries, "lookups/s", () => {
  for (let index = 0; index < siblingQueries; index += 1) {
    sink ^= nodeOps.nextSibling(siblings[index % siblingCount]!)?.id ?? 0
  }
})
const scannedSiblings = measure(
  "legacy array scan model",
  siblingQueries,
  "lookups/s",
  () => {
    for (let index = 0; index < siblingQueries; index += 1) {
      const node = siblings[index % siblingCount]!
      const position = siblings.indexOf(node)
      sink ^= siblings[position + 1]?.id ?? 0
    }
  },
  3,
)
const siblingSpeedup = scannedSiblings.medianMs / linkedSiblings.medianMs
requireAtLeast(siblingSpeedup, 10, "Linked sibling speedup")

const batchSize = 25_000
const batchNative = new MemoryNativeRenderer()
batchNative.createElement(1, "text")
const batched = wrapWithBatching(batchNative)
let batchGeneration = 0
const batchedMutations = measure(
  "JS batch enqueue + atomic apply",
  batchSize,
  "mutations/s",
  () => {
    batchGeneration += 1
    for (let index = 0; index < batchSize; index += 1) {
      batched.setText(1, `${batchGeneration & 1}:${index}`)
    }
    sink ^= batched.flushMutations().length
  },
)
requireAtLeast(rate(batchedMutations), 100_000, "JS batch throughput")

const sampleCount = 1_000_000
const samples = new Float32Array(sampleCount)
for (let index = 0; index < samples.length; index += 1) samples[index] = (index % 127) / 127
const audioNative = new MemoryNativeRenderer()
const audio = createAudioFrames(audioNative)
audio.configure(48_000, 2, sampleCount)
const audioEnqueue = measure(
  "chunked Float32 audio enqueue",
  sampleCount,
  "samples/s",
  () => {
    audio.clear()
    sink ^= audio.enqueue(samples).queuedFrames
  },
  5,
)
requireAtLeast(rate(audioEnqueue), 1_000_000, "Audio enqueue throughput")

const measurements = [linkedSiblings, scannedSiblings, batchedMutations, audioEnqueue]
console.log("\nJavaScript hot-path benchmark (median, warm JIT)")
console.table(
  measurements.map((measurement) => ({
    case: measurement.name,
    medianMs: measurement.medianMs.toFixed(3),
    [measurement.unit]: Math.round(rate(measurement)).toLocaleString(),
  })),
)
console.log(
  `Linked sibling lookup speedup over the former scan model: ${siblingSpeedup.toFixed(1)}x`,
)
console.log("Text-only Vue patch: 1 mutation + 1 commit (formerly 5 mutations + 1 commit)")
if (sink === Number.MIN_SAFE_INTEGER) console.log("unreachable", sink)
