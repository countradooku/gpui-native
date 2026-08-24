import { createWindow } from "gpui-vue"

import ControllerApp from "./ControllerApp.vue"
import { openInspector } from "./windows.js"

createWindow(ControllerApp, {
  title: "gpui-vue window controller",
  width: 540,
  height: 430,
  minWidth: 480,
  minHeight: 390,
})
openInspector()
