import { act, createRef, useState, type ReactNode } from "react"
import { afterEach, expect, it, vi } from "vitest"

import {
  Button,
  Column,
  Row,
  ScrollView,
  Text,
  TextInput,
  TextArea,
  View,
  GpuiDiv,
  GpuiTextElement,
  GpuiInput,
  GpuiTextarea,
  MotionView,
  MotionDiv,
  motion,
} from "../src/components.js"
import type { GpuiPublicInstance } from "../src/renderer.js"
import { mountGpui } from "../src/testing.js"

Reflect.set(globalThis, "IS_REACT_ACT_ENVIRONMENT", true)
const roots: ReturnType<typeof mountGpui>[] = []
afterEach(() => act(() => roots.splice(0).forEach((root) => root.unmount())))
function mount(node: ReactNode) {
  let root!: ReturnType<typeof mountGpui>
  act(() => {
    root = mountGpui(node)
  })
  roots.push(root)
  return root
}

it("keeps legacy component names as identity aliases", () => {
  expect(View).toBe(GpuiDiv)
  expect(Text).toBe(GpuiTextElement)
  expect(TextInput).toBe(GpuiInput)
  expect(TextArea).toBe(GpuiTextarea)
  expect(MotionView).toBe(MotionDiv)
  expect(motion.View).toBe(MotionView)
})

it("keeps one host per layout and forwards styles, input updates and refs", () => {
  const element = createRef<GpuiPublicInstance>()
  function App() {
    const [value, setValue] = useState("hello")
    return (
      <Column testId="column" style={{ gap: 12 }}>
        <Row testId="row" style={{ flexDirection: "row-reverse" }}>
          <Text ref={element} testId="label">
            Label
          </Text>
          <TextInput testId="input" value={value} onChange={(e) => setValue(e.value ?? "")} />
        </Row>
        <ScrollView horizontal testId="scroll" style={{ width: 100 }}>
          <Text>Scrollable</Text>
        </ScrollView>
      </Column>
    )
  }
  const root = mount(<App />)
  expect(root.findByTestId("column").node.children.length).toBe(2)
  expect(root.findByTestId("row").prop("style")).toMatchObject({ flexDirection: "row-reverse" })
  expect(element.current?.id).toBe(root.findByTestId("label").id)
  expect(root.findByTestId("scroll").prop("style")).toMatchObject({
    overflowX: "scroll",
    width: 100,
  })
  act(() => root.findByTestId("input").trigger("change", { value: "updated" }))
  expect(root.findByTestId("input").prop("value")).toBe("updated")
})

it("activates once per gesture, composes raw events and cancels pending Space", () => {
  const element = createRef<GpuiPublicInstance>()
  const presses = vi.fn<(...args: unknown[]) => void>()
  const clicks = vi.fn<(...args: unknown[]) => void>()
  const keys = vi.fn<(...args: unknown[]) => void>()
  const renderButton = (disabled: boolean) => (
    <Button
      ref={element}
      testId="button"
      disabled={disabled}
      onPress={presses}
      onClick={clicks}
      onKeyDown={keys}
      autoFocus
      tabIndex={2}
      style={{ padding: 9, background: "#123456", hover: { opacity: 0.8 } }}
    >
      <Text>Press me</Text>
    </Button>
  )
  const root = mount(renderButton(false))
  const button = root.findByTestId("button")
  expect(element.current?.id).toBe(button.id)
  expect(button.prop("role")).toBe("button")
  expect(button.prop("style")).toMatchObject({ padding: 9, hover: { opacity: 0.8 } })
  button.click()
  button.trigger("keyDown", { key: "Enter" })
  button.trigger("keyDown", { key: "enter", isHeld: true })
  button.trigger("keyDown", {
    key: "enter",
    modifiers: { ctrl: true, alt: false, shift: false, cmd: false },
  })
  button.trigger("keyUp", { key: "space" })
  expect(presses).toHaveBeenCalledTimes(2)
  button.trigger("keyDown", { key: "space" })
  act(() => root.render(renderButton(false)))
  button.trigger("keyUp", { key: "space" })
  expect(presses).toHaveBeenCalledTimes(3)
  expect(clicks).toHaveBeenCalledOnce()
  expect(keys).toHaveBeenCalledTimes(4)
  button.trigger("keyDown", { key: " " })
  button.trigger("blur")
  button.trigger("keyUp", { key: " " })
  button.trigger("keyDown", { key: "space" })
  act(() => root.render(renderButton(true)))
  expect(button.prop("tabIndex")).toBe(-1)
  expect(button.prop("autoFocus")).toBe(false)
  button.click()
  button.trigger("keyDown", { key: "enter" })
  act(() => root.render(renderButton(false)))
  button.trigger("keyUp", { key: "space" })
  expect(presses).toHaveBeenCalledTimes(3)
  expect(button.prop("tabIndex")).toBe(2)
  button.click()
  expect(presses).toHaveBeenCalledTimes(4)
})
