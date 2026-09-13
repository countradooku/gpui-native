import { act, useState } from "react"
import { expect, it, vi } from "vitest"

import { Checkbox, DataTable, Dialog, Input } from "../src/kit.js"
import { mountGpui } from "../src/testing.js"
Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true)
it("reconciles Kit values through the native batch protocol and preserves raw compound props", () => {
  const raw = vi.fn<(event: import("@gpui-native/runtime").EventPayload) => void>()
  function App() {
    const [checked, setChecked] = useState(false)
    const [value, setValue] = useState("first")
    return (
      <Dialog open>
        <Checkbox testId="check" checked={checked} onValueChange={setChecked} onChange={raw} />
        <Input testId="input" value={value} onValueChange={setValue} />
        <DataTable
          testId="table"
          columns={[{ key: "name" }]}
          rows={[{ name: 'A "quoted" value' }]}
        />
      </Dialog>
    )
  }
  let root!: ReturnType<typeof mountGpui>
  act(() => {
    root = mountGpui(<App />)
  })
  try {
    expect(root.findByTestId("check").node.tag).toBe("kit-checkbox")
    act(() => root.findByTestId("check").trigger("change", { value: "true" }))
    act(() => root.findByTestId("input").trigger("change", { value: '"new\\nvalue"' }))
    expect(root.findByTestId("check").prop("checked")).toBe(true)
    expect(root.findByTestId("input").prop("value")).toBe("new\nvalue")
    expect(root.findByTestId("table").prop("rows")).toEqual([{ name: 'A "quoted" value' }])
    expect(raw).toHaveBeenCalledOnce()
    expect(root.findByTestId("check").prop("onValueChange")).toBeUndefined()
  } finally {
    act(() => root.unmount())
  }
})
