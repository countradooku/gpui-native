import {
  GpuiDiv,
  GpuiInput,
  GpuiTextElement,
  useWindowSize,
  type StyleDesc,
} from "@gpui-native/react"
import { useState } from "react"
const button: StyleDesc = {
  padding: 14,
  backgroundColor: "#30466e",
  borderRadius: 8,
  cursor: "pointer",
}
export default function App() {
  const [count, setCount] = useState(0)
  const [name, setName] = useState("React")
  const size = useWindowSize()
  return (
    <GpuiDiv
      style={{
        width: "100%",
        height: "100%",
        padding: 32,
        gap: 20,
        display: "flex",
        flexDirection: "column",
        backgroundColor: "#111827",
        color: "#f8fafc",
      }}
    >
      <GpuiTextElement style={{ fontSize: 30, fontWeight: 700 }}>Hello, {name}</GpuiTextElement>
      <GpuiInput
        testId="name"
        value={name}
        onChange={(event) => setName(event.value ?? "")}
        style={{ width: 280, padding: 12, backgroundColor: "#1e293b" }}
      />
      <GpuiTextElement testId="count" style={{ fontSize: 48 }}>
        {count}
      </GpuiTextElement>
      <GpuiDiv style={{ display: "flex", flexDirection: "row", gap: 12 }}>
        <GpuiDiv
          testId="decrement"
          role="button"
          tabIndex={0}
          style={button}
          onClick={() => setCount((value) => value - 1)}
        >
          −
        </GpuiDiv>
        <GpuiDiv
          testId="increment"
          role="button"
          tabIndex={0}
          style={button}
          onClick={() => setCount((value) => value + 1)}
        >
          +
        </GpuiDiv>
        <GpuiDiv style={button} onClick={() => setCount(0)}>
          Reset
        </GpuiDiv>
      </GpuiDiv>
      <GpuiTextElement>
        {size.width} × {size.height} · rendered by GPUI
      </GpuiTextElement>
    </GpuiDiv>
  )
}
