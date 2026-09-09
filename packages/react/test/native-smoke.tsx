import assert from "node:assert/strict"

import { act, useState } from "react"

import { GpuiCode, GpuiCanvas, GpuiMarkdown } from "../src/components.js"
import { createTestRoot, hasNativeTestRenderer } from "../src/testing.js"
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true)
if (!hasNativeTestRenderer) {
  console.log("React GPU smoke skipped: native test renderer is unavailable")
  process.exit(0)
}
const root = createTestRoot({ width: 640, height: 480 })
function App() {
  const [count, setCount] = useState(0)
  const [value, setValue] = useState("hello")
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: 640,
        height: 480,
        backgroundColor: "#fff",
        color: "#111",
      }}
    >
      <div
        testId="button"
        role="button"
        aria-label="Increment React counter"
        onClick={() => setCount((value) => value + 1)}
        style={{ width: 200, height: 40 }}
      >
        Count {count}
      </div>
      <textarea
        testId="input"
        value={value}
        onChange={(event) => setValue(event.value ?? "")}
        style={{ width: 300, height: 80 }}
      />
      <GpuiCode code="const react = true" language="javascript" style={{ height: 60 }} />
      <GpuiMarkdown source="# React native" style={{ height: 70 }} />
      <GpuiCanvas
        commands={[{ type: "rect", x: 0, y: 0, width: 80, height: 30, fill: "#f00" }]}
        style={{ height: 50 }}
      />
    </div>
  )
}
try {
  act(() => root.render(<App />))
  const button = root.findByTestId("button")
  const bounds = root.renderer.getElementBounds(button.id)
  assert(bounds && bounds[2]! > 0)
  act(() => root.renderer.nativeSimulateClick(bounds[0]! + 5, bounds[1]! + 5))
  await root.flush()
  assert.equal(root.findByTestId("button").text, "Count 1")
  assert(
    root.renderer.getPaintedText().includes("Count 1"),
    "interpolations must share one native text run",
  )
  assert(JSON.stringify(root.renderer.getA11yTree()).includes("Increment React counter"))
  act(() => root.renderer.nativeSimulateKeystrokes(root.findByTestId("input").id, "end enter"))
  await root.flush()
  assert.equal(root.findByTestId("input").prop("value"), "hello\n")
  assert(root.renderer.getPaintedText().some((text) => text.includes("React")))
} finally {
  act(() => root.unmount())
}
assert.equal(root.renderer.getRetainedElementCount(), 0)
console.log("React GPU layout, click, input, accessibility, native components and cleanup passed")
