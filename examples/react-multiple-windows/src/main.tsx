import { createWindow, useGpuiWindow, type GpuiWindowRoot } from "@gpui-native/react"
import { useState } from "react"
function Inspector() {
  const window = useGpuiWindow()
  return (
    <div style={{ padding: 24, color: "#fff", backgroundColor: "#172033", height: "100%" }}>
      <text>Independent React root</text>
      <div onClick={() => window.setTitle("Inspector updated")}>Rename this window</div>
    </div>
  )
}
function Controller() {
  const [windows, setWindows] = useState<GpuiWindowRoot[]>([])
  return (
    <div
      style={{
        padding: 24,
        gap: 20,
        display: "flex",
        flexDirection: "column",
        color: "#fff",
        backgroundColor: "#111827",
        height: "100%",
      }}
    >
      <text style={{ fontSize: 24 }}>React windows</text>
      <div
        onClick={() =>
          setWindows((windows) => [
            ...windows,
            createWindow(<Inspector />, { title: "React inspector", width: 400, height: 300 }),
          ])
        }
      >
        Open inspector
      </div>
      <div
        onClick={() => {
          windows.forEach((window) => window.close())
          setWindows([])
        }}
      >
        Close inspectors ({windows.length})
      </div>
    </div>
  )
}
createWindow(<Controller />, { title: "React window controller", width: 600, height: 400 })
