import { render } from "@gpui-native/vue"
import { installWebGPU } from "@gpui-native/vue/webgpu-native"
import { h } from "vue"

import App from "./App.vue"
await installWebGPU()
const gpu = navigator.gpu
render(h(App, { gpu }), { title: "GPUI Vue WebGPU", width: 820, height: 840 })
