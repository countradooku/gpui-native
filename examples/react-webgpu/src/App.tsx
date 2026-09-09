import { GpuiCanvas, useGPUCanvas } from "@gpui-native/react"
import { useEffect, useState } from "react"

import { startWebGPUScene } from "../../shared/webgpu-scene.js"

export default function App({ gpu }: { gpu: GPU }) {
  const canvas = useGPUCanvas({ width: 640, height: 400 })
  const [error, setError] = useState("")
  useEffect(() => {
    if (!canvas) return
    let cancelled = false
    let stop: (() => void) | undefined
    void startWebGPUScene(canvas, gpu, (error) => setError(String(error)))
      .then((dispose) => {
        if (cancelled) dispose()
        else stop = dispose
      })
      .catch((error) => {
        if (!cancelled) setError(String(error))
      })
    return () => {
      cancelled = true
      stop?.()
    }
  }, [canvas, gpu])
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        padding: 24,
        gap: 16,
        flexDirection: "column",
        backgroundColor: "#070b17",
        color: "#e8efff",
      }}
    >
      <text style={{ fontSize: 26 }}>React · WebGPU canvas</text>
      <text>WGSL shaders · 4× antialiasing · bounded asynchronous presentation</text>
      {canvas && (
        <GpuiCanvas
          source={canvas.id}
          testId="webgpu-canvas"
          style={{ width: 640, height: 400 }}
          commands={[
            {
              type: "rect",
              x: 12,
              y: 12,
              width: 616,
              height: 376,
              radius: 12,
              stroke: "#6686b0",
              strokeWidth: 1,
            },
          ]}
        />
      )}
      {error && <text style={{ color: "#ff8888" }}>{error}</text>}
    </div>
  )
}
