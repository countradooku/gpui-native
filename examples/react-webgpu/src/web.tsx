import { render } from "@gpui-native/react"

import App from "./App.js"
if (!navigator.gpu) throw new Error("This browser does not expose WebGPU")
render(<App gpu={navigator.gpu} />, { title: "GPUI React WebGPU", width: 700, height: 540 })
