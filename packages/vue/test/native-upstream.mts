import assert from "node:assert/strict"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { h } from "@vue/runtime-core"

import { createTestRoot, hasNativeTestRenderer } from "../src/native-testing.js"

if (!hasNativeTestRenderer) {
  console.log("GPU parity smoke skipped: native test renderer is unavailable")
  process.exit(0)
}
const root = createTestRoot({ width: 480, height: 360 })
let clicks = 0
let changed = ""
let submitted = 0
try {
  root.render(
    h("div", { style: { display: "flex", flexDirection: "column", width: 480 } }, [
      h(
        "div",
        {
          role: "button",
          "aria-label": "Delete note",
          onClick: () => clicks++,
          style: { width: 100, height: 30 },
        },
        "Delete",
      ),
      h("text", {}, ["Hello ", "world"]),
      h("img", { alt: "Missing image", style: { width: 100, height: 30 } }),
      h("textarea", {
        value: "hello",
        onChange: (event: { value?: string }) => {
          changed = event.value ?? ""
        },
        style: { width: 200, height: 70 },
      }),
    ]),
  )
  const dump = JSON.stringify(root.renderer.getA11yTree())
  assert(dump.includes("Delete note"), "button label must reach GPUI accessibility")
  assert(dump.includes("Hello world"), "interpolated text must share one accessible value")
  assert(dump.includes("Missing image"), "empty image must retain its accessible label")
  const container = root.host.root.children[0]!
  assert(container.kind === "element")
  const button = container.children[0]!
  const bounds = root.renderer.getElementBounds(button.id)!
  root.renderer.nativeSimulateClick(bounds[0]! + 5, bounds[1]! + 5)
  assert.equal(clicks, 1)
  const textarea = container.children[3]!
  root.renderer.nativeSimulateKeystrokes(textarea.id, "end enter")
  assert.equal(changed, "hello\n", "textarea Enter should insert a newline")

  root.render(
    h("textarea", {
      value: "hello",
      onSubmit: () => submitted++,
      onChange: (event: { value?: string }) => {
        changed = event.value ?? ""
      },
      style: { width: 200, height: 70 },
    }),
  )
  const composer = root.host.root.children[0]!
  root.renderer.nativeSimulateKeystrokes(composer.id, "enter")
  assert.equal(submitted, 1)
  root.renderer.nativeSimulateKeystrokes(composer.id, "shift-enter")
  assert.equal(changed, "hello\n")
  const output = mkdtempSync(join(tmpdir(), "gpui-parity-"))
  try {
    root.render(
      h(
        "text",
        { style: { fontSize: 24, color: "white", textDecoration: "none" } },
        "Decorated text",
      ),
    )
    root.renderer.captureScreenshot(join(output, "plain.png"))
    root.render(
      h(
        "text",
        { style: { fontSize: 24, color: "white", textDecoration: "underline" } },
        "Decorated text",
      ),
    )
    root.renderer.captureScreenshot(join(output, "underline.png"))
    assert(
      !readFileSync(join(output, "plain.png")).equals(readFileSync(join(output, "underline.png"))),
      "underline must change painted pixels",
    )
    root.render(h("div", { style: { width: 100, height: 100, background: "red" } }))
    root.renderer.captureScreenshot(join(output, "solid.png"))
    root.render(
      h("div", {
        style: {
          width: 100,
          height: 100,
          background: {
            type: "linear-gradient",
            angle: 90,
            stops: [
              { color: "red", position: 0 },
              { color: "blue", position: 1 },
            ],
          },
        },
      }),
    )
    root.renderer.captureScreenshot(join(output, "gradient.png"))
    assert(
      !readFileSync(join(output, "solid.png")).equals(readFileSync(join(output, "gradient.png"))),
      "gradient must change painted pixels",
    )
  } finally {
    rmSync(output, { recursive: true, force: true })
  }
  console.log("GPU parity smoke passed: accessibility, primary click, textarea newline and submit")
} finally {
  root.unmount()
}
