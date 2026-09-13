import { Canvas, useGPUCanvas, useWindowSize } from "@gpui-native/react"
import { useEffect, useState } from "react"

import { startThreeScene } from "../../shared/three-scene.js"
import { startWebGPUScene } from "../../shared/webgpu-scene.js"

function Canvases({ gpu, width }: { gpu: GPU; width: number }) {
  const compute = useGPUCanvas({ width, height: 300 })
  const three = useGPUCanvas({ width, height: 300 })
  const [error, setError] = useState("")
  useEffect(() => {
    if (!compute || !three) return
    let cancelled = false
    const stops: (() => void)[] = []
    for (const [canvas, start] of [
      [compute, startWebGPUScene],
      [three, startThreeScene],
    ] as const) {
      void start(canvas, gpu, (error) => setError(String(error)))
        .then((stop) => {
          if (cancelled) stop()
          else stops.push(stop)
        })
        .catch((error) => {
          if (!cancelled) setError(String(error))
        })
    }
    return () => {
      cancelled = true
      for (const stop of stops) stop()
    }
  }, [compute, three, gpu])
  return (
    <div style={{ display: "flex", flexDirection: "column", flexShrink: 0, gap: 12 }}>
      <text>Compute + WGSL · 4× MSAA</text>
      {compute && (
        <Canvas
          source={compute.id}
          testId="webgpu-canvas"
          style={{ width, height: 300, flexShrink: 0 }}
        />
      )}
      <text>Textured Three.js · depth + 4× MSAA</text>
      {three && (
        <Canvas
          source={three.id}
          testId="three-canvas"
          style={{ width, height: 300, flexShrink: 0 }}
        />
      )}
      {error && <text style={{ color: "#ff8888" }}>{error}</text>}
    </div>
  )
}
export default function App({ gpu }: { gpu: GPU }) {
  const [visible, setVisible] = useState(true)
  const [wide, setWide] = useState(false)
  const size = useWindowSize()
  const canvasWidth = Math.max(1, Math.min(wide ? 720 : 480, size.width - 48))
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        overflow: "scroll",
        padding: 24,
        gap: 12,
        display: "flex",
        flexDirection: "column",
        backgroundColor: "#070b17",
        color: "#e8efff",
      }}
    >
      <text style={{ fontSize: 26 }}>React · shared GPU canvases</text>
      <div
        style={{ display: "flex", flexDirection: "row", flexWrap: "wrap", flexShrink: 0, gap: 12 }}
      >
        <div
          role="button"
          style={{ padding: 10, backgroundColor: "#23304a", borderRadius: 6 }}
          onClick={() => setVisible(!visible)}
        >
          {visible ? "Unmount canvases" : "Mount canvases"}
        </div>
        <div
          role="button"
          style={{ padding: 10, backgroundColor: "#23304a", borderRadius: 6 }}
          onClick={() => setWide(!wide)}
        >
          Resize canvases
        </div>
      </div>
      {visible && <Canvases gpu={gpu} width={canvasWidth} />}
    </div>
  )
}
