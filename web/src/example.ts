import { initGpuiWeb } from "gpui-vue"

const example = document.body.dataset.example
const loaders: Record<string, () => Promise<unknown>> = {
  "audio-buffer": () => import("../../examples/audio-buffer/src/main.js"),
  canvas: () => import("../../examples/canvas/src/main.js"),
  counter: () => import("../../examples/counter/src/main.js"),
  "market-stream": () => import("../../examples/market-stream/src/main.js"),
  "motion-timeline": () => import("../../examples/motion-timeline/src/main.js"),
  "multiple-windows": () => import("../../examples/multiple-windows/src/main.js"),
}

async function start(): Promise<void> {
  if (example === undefined || loaders[example] === undefined) {
    throw new Error(`Unknown gpui-vue example: ${example ?? "missing"}`)
  }

  await initGpuiWeb()
  await loaders[example]()

  const loading = document.querySelector<HTMLElement>("#loading")
  const reveal = (): boolean => {
    if (document.querySelector("canvas") === null) return false
    loading?.classList.add("hidden")
    return true
  }
  if (!reveal()) {
    const observer = new MutationObserver(() => {
      if (reveal()) observer.disconnect()
    })
    const body = document.body
    if (body !== null) observer.observe(body, { childList: true })
  }
}

start().catch((error: unknown) => {
  const loading = document.querySelector<HTMLElement>("#loading")
  if (loading !== null) {
    loading.classList.add("error")
    loading.textContent = error instanceof Error ? error.message : String(error)
  }
  console.error(error)
})
