import { defineComponent, h, ref } from "@vue/runtime-core"
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
  mountGpui,
  useElementRef,
} from "../src/index.js"

const roots: ReturnType<typeof mountGpui>[] = []
afterEach(() => roots.splice(0).forEach((root) => root.unmount()))
function mount(component: Parameters<typeof mountGpui>[0]) {
  const root = mountGpui(component)
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

it("keeps one host per layout and forwards styling, model updates and native refs", async () => {
  const value = ref("hello")
  const horizontal = ref(false)
  const element = useElementRef()
  const changed = vi.fn<(...args: unknown[]) => void>()
  const root = mount(
    defineComponent(
      () => () =>
        h(Column, { testId: "column", style: { gap: 12 } }, () => [
          h(Row, { testId: "row", style: { flexDirection: "row-reverse" } }, () => [
            h(Text, { testId: "label", ref: element }, () => "Label"),
            h(TextInput, {
              testId: "input",
              modelValue: value.value,
              onChange: changed,
              "onUpdate:modelValue": (next: string) => {
                value.value = next
              },
            }),
          ]),
          h(
            ScrollView,
            { testId: "scroll", horizontal: horizontal.value, style: { height: 100 } },
            () => [h(Text, null, () => "Scrollable")],
          ),
        ]),
    ),
  )
  expect(root.findByTestId("column").node.children.length).toBe(2)
  expect(root.findByTestId("row").prop("style")).toMatchObject({ flexDirection: "row-reverse" })
  expect(element.value?.id).toBe(root.findByTestId("label").id)
  expect(root.findByTestId("scroll").prop("style")).toMatchObject({
    overflowY: "scroll",
    height: 100,
  })
  root.findByTestId("input").trigger("change", { value: "updated" })
  horizontal.value = true
  await root.flush()
  expect(value.value).toBe("updated")
  expect(changed).toHaveBeenCalledOnce()
  expect(root.findByTestId("input").prop("value")).toBe("updated")
  expect(root.findByTestId("scroll").prop("style")).toMatchObject({
    overflowX: "scroll",
    overflowY: "hidden",
  })
})

it("activates once per gesture, composes raw events and cancels pending Space", async () => {
  const disabled = ref(false)
  const presses = vi.fn<(...args: unknown[]) => void>()
  const clicks = vi.fn<(...args: unknown[]) => void>()
  const keys = vi.fn<(...args: unknown[]) => void>()
  const element = useElementRef()
  const root = mount(
    defineComponent(
      () => () =>
        h(
          Button,
          {
            ref: element,
            testId: "button",
            disabled: disabled.value,
            onPress: presses,
            onClick: clicks,
            onKeyDown: keys,
            autoFocus: true,
            tabIndex: 2,
            style: { padding: 9, background: "#123456", hover: { opacity: 0.8 } },
          },
          () => [h(Text, null, () => "Press me")],
        ),
    ),
  )
  const button = root.findByTestId("button")
  expect(element.value?.id).toBe(button.id)
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
  await root.flush()
  button.trigger("keyUp", { key: "space" })
  expect(presses).toHaveBeenCalledTimes(3)
  expect(clicks).toHaveBeenCalledOnce()
  expect(keys).toHaveBeenCalledTimes(4)
  button.trigger("keyDown", { key: " " })
  button.trigger("blur")
  button.trigger("keyUp", { key: " " })
  button.trigger("keyDown", { key: "space" })
  disabled.value = true
  await root.flush()
  expect(button.prop("tabIndex")).toBe(-1)
  expect(button.prop("autoFocus")).toBe(false)
  button.click()
  button.trigger("keyDown", { key: "enter" })
  disabled.value = false
  await root.flush()
  button.trigger("keyUp", { key: "space" })
  expect(presses).toHaveBeenCalledTimes(3)
  expect(button.prop("tabIndex")).toBe(2)
  button.click()
  expect(presses).toHaveBeenCalledTimes(4)
})
