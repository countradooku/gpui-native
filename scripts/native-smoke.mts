import { createNativeRenderer } from "../packages/gpui-vue/dist/index.js"

function mountAndClose(label: string): void {
  const renderer = createNativeRenderer()
  renderer.init({ headless: true })
  renderer.applyBatch(
    JSON.stringify([
      ["createElement", 1, "div"],
      ["createElement", 2, "text"],
      ["setText", 2, label],
      ["appendChild", 1, 2],
      ["setRoot", 1],
    ]),
  )

  if (!renderer.getAllText().includes(label)) {
    throw new Error(`Native renderer did not retain ${JSON.stringify(label)}`)
  }

  renderer.close()
  if (renderer.isInitialized()) throw new Error("Native renderer did not close")
}

mountAndClose("first native root")
mountAndClose("second native root")

console.log("Native addon load and close/reopen smoke passed")
