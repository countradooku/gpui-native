import { render } from "@gpui-native/vue"
import { h } from "vue"

import App from "./App.vue"
if (!navigator.gpu) throw new Error("This browser does not expose WebGPU")
render(h(App, { gpu: navigator.gpu }), { title: "GPUI Vue WebGPU", width: 700, height: 540 })
