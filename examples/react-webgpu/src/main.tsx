import { render } from "@gpui-native/react"
import { installWebGPU } from "@gpui-native/react/webgpu-native"

import App from "./App.js"
await installWebGPU()
const gpu = navigator.gpu
render(<App gpu={gpu} />, { title: "GPUI React WebGPU", width: 820, height: 840 })
