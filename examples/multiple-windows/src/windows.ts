import { createWindow, type GpuiWindowRoot } from "gpui-vue"

import InspectorApp from "./InspectorApp.vue"
import { inspectorOpen } from "./state.js"

let inspectorRoot: GpuiWindowRoot | undefined

export function openInspector(): void {
  if (inspectorRoot !== undefined) return
  inspectorRoot = createWindow(InspectorApp, {
    title: "gpui-vue inspector window",
    width: 430,
    height: 390,
    minWidth: 380,
    minHeight: 340,
  })
  inspectorOpen.value = true
}

export function closeInspector(): void {
  inspectorRoot?.close()
  inspectorRoot = undefined
  inspectorOpen.value = false
}
