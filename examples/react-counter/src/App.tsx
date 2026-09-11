import {
  Button,
  Column,
  Row,
  TextInput,
  Text,
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
    <Column
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
      <Text style={{ fontSize: 30, fontWeight: 700 }}>Hello, {name}</Text>
      <TextInput
        testId="name"
        value={name}
        onChange={(event) => setName(event.value ?? "")}
        style={{ width: 280, padding: 12, backgroundColor: "#1e293b" }}
      />
      <Text testId="count" style={{ fontSize: 48 }}>
        {count}
      </Text>
      <Row style={{ gap: 12 }}>
        <Button testId="decrement" style={button} onPress={() => setCount((value) => value - 1)}>
          −
        </Button>
        <Button testId="increment" style={button} onPress={() => setCount((value) => value + 1)}>
          +
        </Button>
        <Button style={button} disabled={count === 0} onPress={() => setCount(0)}>
          Reset
        </Button>
      </Row>
      <Text>
        {size.width} × {size.height} · rendered by GPUI
      </Text>
    </Column>
  )
}
