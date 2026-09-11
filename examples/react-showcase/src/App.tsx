import {
  View,
  TextInput,
  Code,
  Markdown,
  Canvas,
  VirtualList,
  MotionView,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
  Combobox,
  ComboboxInput,
  ComboboxContent,
  ComboboxList,
  ComboboxItem,
  ComboboxEmpty,
  useTextSearch,
  useGpuiTimeline,
  type StyleDesc,
} from "@gpui-native/react"
import { useState } from "react"
const panel: StyleDesc = {
  padding: 16,
  gap: 10,
  borderRadius: 10,
  backgroundColor: "#1e293b",
  display: "flex",
  flexDirection: "column",
}
export default function App() {
  const [query, setQuery] = useState("native")
  const search = useTextSearch({ query })
  const timeline = useGpuiTimeline()
  return (
    <View
      style={{
        width: "100%",
        height: "100%",
        padding: 24,
        gap: 16,
        display: "flex",
        flexDirection: "column",
        overflow: "scroll",
        backgroundColor: "#0f172a",
        color: "#e2e8f0",
      }}
    >
      <text style={{ fontSize: 28, fontWeight: 700 }}>React × GPUI</text>
      <div style={panel}>
        <Tooltip>
          <TooltipTrigger style={{ cursor: "pointer" }}>
            Hover or focus for a native tooltip
          </TooltipTrigger>
          <TooltipContent style={{ padding: 12 }}>React state, GPUI positioning</TooltipContent>
        </Tooltip>
        <Select defaultValue="react">
          <SelectTrigger style={{ padding: 10 }}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent style={{ padding: 12 }}>
            <SelectItem value="react">React</SelectItem>
            <SelectItem value="vue">Vue</SelectItem>
            <SelectItem value="future" disabled>
              More adapters soon
            </SelectItem>
          </SelectContent>
        </Select>
        <Combobox
          items={["Canvas", "Code", "Markdown", "Virtual list", "Native text"]}
          autoHighlight
        >
          <ComboboxInput placeholder="Find a component" style={{ padding: 12 }} />
          <ComboboxContent style={{ padding: 12 }}>
            <ComboboxList>
              {(item) => (
                <ComboboxItem
                  key={item}
                  value={item}
                  style={({ highlighted }) => ({
                    padding: 8,
                    backgroundColor: highlighted ? "#334155" : "#1e293b",
                  })}
                >
                  {item}
                </ComboboxItem>
              )}
            </ComboboxList>
            <ComboboxEmpty>No matches</ComboboxEmpty>
          </ComboboxContent>
        </Combobox>
      </div>
      <div style={panel}>
        <TextInput
          value={query}
          onChange={(event) => setQuery(event.value ?? "")}
          placeholder="Search text"
        />
        <div {...search.props}>
          <Markdown
            source={
              "# Native text\nReact uses the same native selection and search pipeline as Vue."
            }
          />
        </div>
        <div onClick={search.next}>Next match ({search.total})</div>
      </div>
      <Code
        code={"function App() {\n  return <text>Hello, native world</text>\n}"}
        language="tsx"
        style={panel}
      />
      <Canvas
        style={{ height: 90 }}
        commands={[
          { type: "rect", x: 0, y: 0, width: 160, height: 70, radius: 12, fill: "#38bdf8" },
          { type: "circle", cx: 210, cy: 35, radius: 30, fill: "#a78bfa" },
        ]}
      />
      <MotionView
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 1 }}
        style={panel}
      >
        Native motion, with no React updates per frame
      </MotionView>
      <div onClick={() => timeline.pause()}>Pause timeline</div>
      <div onClick={() => timeline.play()}>Resume timeline</div>
      <VirtualList estimatedItemHeight={30} style={{ height: 160 }}>
        {Array.from({ length: 100 }, (_, i) => (
          <text key={i} style={{ height: 30 }}>
            Native row {i + 1}
          </text>
        ))}
      </VirtualList>
    </View>
  )
}
