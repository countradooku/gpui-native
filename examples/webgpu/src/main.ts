import { render } from "@gpui-native/vue"
import { createNativeGPU } from "@gpui-native/vue/webgpu-native"
import { h } from "vue"

import App from "./App.vue"
const gpu = await createNativeGPU()
render(h(App, { gpu }), { title: "GPUI Vue WebGPU", width: 700, height: 540 })
