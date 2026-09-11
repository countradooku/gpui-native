import { render } from "@gpui-native/svelte"
import { installWebGPU } from "@gpui-native/svelte/webgpu-native"

import App from "./App.svelte"
await installWebGPU()
render(App, { props: { gpu: navigator.gpu }, title: "Svelte WebGPU", width: 760, height: 560 })
