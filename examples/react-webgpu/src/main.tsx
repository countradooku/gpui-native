import { render } from "@gpui-native/react"
import { createNativeGPU } from "@gpui-native/react/webgpu-native"

import App from "./App.js"
const gpu = await createNativeGPU()
render(<App gpu={gpu} />, { title: "GPUI React WebGPU", width: 700, height: 540 })
