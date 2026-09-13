import assert from "node:assert/strict"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { createRequire } from "node:module"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { PNG } from "pngjs"

import { kitExamples as cases } from "../../../examples/kit/src/catalog.js"
import { KIT_COMPONENTS } from "../src/kit.js"
const { TestGpuiRenderer } = createRequire(import.meta.url)(
  "@gpui-native/core",
) as typeof import("@gpui-native/core")

const renderer = new TestGpuiRenderer(420, 280)
renderer.applyBatch(
  JSON.stringify([
    ["createElement", 1, "div"],
    [
      "setStyle",
      1,
      {
        width: 420,
        height: 280,
        padding: 24,
        display: "flex",
        flexDirection: "column",
        gap: 20,
        backgroundColor: "#ffffff",
      },
    ],
    ["createElement", 2, "kit-button"],
    ["setCustomPropValue", 2, "label", "Save changes"],
    ["setCustomPropValue", 2, "variant", "primary"],
    ["setEventListener", 2, "click", true],
    ["appendChild", 1, 2],
    ["createElement", 3, "kit-checkbox"],
    ["setCustomPropValue", 3, "label", "Enable notifications"],
    ["setCustomPropValue", 3, "checked", false],
    ["setEventListener", 3, "change", true],
    ["appendChild", 1, 3],
    ["setRoot", 1],
  ]),
)
renderer.flush()
const button = renderer.getElementBounds(2)
const checkbox = renderer.getElementBounds(3)
assert(button && checkbox, "Kit controls must participate in retained bounds")
assert(button[2] > 0 && button[3] > 0)
renderer.simulateClick(button[0] + 12, button[1] + button[3] / 2)
assert.equal(
  renderer.drainEvents().filter((event) => event.elementId === 2 && event.eventType === "click")
    .length,
  1,
)
renderer.simulateClick(checkbox[0] + 8, checkbox[1] + checkbox[3] / 2)
assert(
  renderer
    .drainEvents()
    .some(
      (event) => event.elementId === 3 && event.eventType === "change" && event.value === "true",
    ),
)
renderer.applyBatch(JSON.stringify([["setCustomPropValue", 2, "disabled", true]]))
renderer.flush()
renderer.simulateClick(button[0] + 12, button[1] + button[3] / 2)
assert.equal(
  renderer.drainEvents().filter((event) => event.elementId === 2 && event.eventType === "click")
    .length,
  0,
)
renderer.applyBatch(JSON.stringify([["setCustomPropValue", 2, "disabled", null]]))
renderer.flush()
renderer.simulateClick(button[0] + 12, button[1] + button[3] / 2)
assert.equal(
  renderer.drainEvents().filter((event) => event.elementId === 2 && event.eventType === "click")
    .length,
  1,
)
renderer.focusElement(2)
renderer.flush()
renderer.simulateKeyDown("space")
renderer.simulateKeyUp("space")
assert.equal(
  renderer.drainEvents().filter((e) => e.elementId === 2 && e.eventType === "click").length,
  1,
  "Kit Button must support keyboard activation after imperative focus",
)
console.log("Kit native control activation and removed-prop reset passed")

assert.deepEqual(
  Object.keys(cases).sort(),
  Object.values(KIT_COMPONENTS).sort(),
  "Every exported Kit component needs a native rendering case",
)
for (const [tag, props] of Object.entries(cases)) {
  console.log(`Rendering ${tag}`)
  renderer.applyBatch(
    JSON.stringify([
      ["destroyElement", 1],
      ["createElement", 1, "div"],
      ["setStyle", 1, { width: 420, height: 280, padding: 12 }],
      ["createElement", 2, tag],
      ["setStyle", 2, { width: 390, height: 250 }],
      ...Object.entries(props).map(([key, value]) => ["setCustomPropValue", 2, key, value]),
      ["createElement", 3, "text"],
      ["setText", 3, "Framework child"],
      ["appendChild", 2, 3],
      ["appendChild", 1, 2],
      ["setRoot", 1],
    ]),
  )
  renderer.flush()
  renderer.flush()
  if (tag.endsWith("chart")) {
    const labels = (props.data as { label: string }[]).map((row) => row.label)
    assert(
      renderer.getPaintedText().some((text) => labels.includes(text)),
      `${tag} must register its canvas labels in the shared text pipeline`,
    )
  }
  if (tag === "kit-shimmer-text") assert(renderer.getPaintedText().includes(String(props.label)))
  if (tag === "kit-pie-chart") {
    const directory = mkdtempSync(join(tmpdir(), "gpui-kit-pie-"))
    try {
      const path = join(directory, "pie.png")
      renderer.captureScreenshot(path)
      const png = PNG.sync.read(readFileSync(path))
      let filledPixels = 0
      for (let offset = 0; offset < png.data.length; offset += 4) {
        if (
          Math.abs(png.data[offset]! - 119) < 3 &&
          Math.abs(png.data[offset + 1]! - 204) < 3 &&
          Math.abs(png.data[offset + 2]! - 153) < 3
        ) {
          filledPixels++
        }
      }
      assert(
        filledPixels > png.width * png.height * 0.02,
        "Pie charts with automatic radius must paint filled slices, not only labels",
      )
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  }
}
renderer.applyBatch(JSON.stringify([["destroyElement", 1]]))
renderer.flush()
assert(
  !renderer.getPaintedText().includes("Framework child"),
  "Unmount must release overlay children",
)
console.log(`Kit catalog: ${Object.keys(cases).length} native components rendered and unmounted`)

renderer.drainEvents()

// Imperative focus must target the native editor, and framework echoes must preserve its caret.
renderer.applyBatch(
  JSON.stringify([
    ["createElement", 1, "div"],
    ["setStyle", 1, { width: 420, height: 280 }],
    ["createElement", 2, "kit-input"],
    ["setCustomPropValue", 2, "value", "start"],
    ["setEventListener", 2, "change", true],
    ["setEventListener", 2, "focus", true],
    ["setEventListener", 2, "keyUp", true],
    ["appendChild", 1, 2],
    ["setRoot", 1],
  ]),
)
renderer.flush()
renderer.focusElement(2)
renderer.flush()
// The headless window can be inactive; GPUI intentionally suppresses focus notifications then.
assert(
  renderer.drainEvents().filter((e) => e.elementId === 2 && e.eventType === "focus").length <= 1,
)

renderer.simulateKeystrokes("end x")
renderer.flush()
let inputChanges = renderer
  .drainEvents()
  .filter((e) => e.elementId === 2 && e.eventType === "change")
assert(
  inputChanges.some((e) => e.value === JSON.stringify("startx")),
  "Kit Input must accept typing after imperative focus",
)
renderer.applyBatch(JSON.stringify([["setCustomPropValue", 2, "value", "startx"]]))
renderer.flush()
renderer.simulateKeystrokes("y")
renderer.flush()
inputChanges = renderer.drainEvents().filter((e) => e.elementId === 2 && e.eventType === "change")
assert(
  inputChanges.some((e) => e.value === JSON.stringify("startxy")),
  "Controlled echoes preserve native caret position",
)
renderer.applyBatch(JSON.stringify([["setCustomPropValue", 2, "readonly", true]]))
renderer.flush()
renderer.simulateKeystrokes("z")
assert.equal(renderer.drainEvents().filter((e) => e.eventType === "change").length, 0)
renderer.applyBatch(JSON.stringify([["setCustomPropValue", 2, "readonly", null]]))
renderer.flush()
renderer.simulateKeystrokes("z")
assert(
  renderer
    .drainEvents()
    .some((e) => e.eventType === "change" && e.value === JSON.stringify("startxyz")),
)
renderer.applyBatch(JSON.stringify([["destroyElement", 1]]))
renderer.flush()
console.log("Kit editor focus, typing, controlled echoes and readonly reset passed")
