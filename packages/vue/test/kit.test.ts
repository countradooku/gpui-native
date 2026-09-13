import { defineComponent, h, ref } from "@vue/runtime-core"
import { expect, it, vi } from "vitest"

import { mountGpui } from "../src/index.js"
import { Checkbox, Slider, Input } from "../src/kit.js"
it("supports Kit default and named Vue models without serializing callback functions", async () => {
  const checked = ref(false),
    range = ref<[number, number]>([10, 20]),
    value = ref("first")
  const raw = vi.fn<(event: import("@gpui-native/runtime").EventPayload) => void>()
  const root = mountGpui(
    defineComponent(
      () => () =>
        h("div", null, [
          h(Checkbox, {
            testId: "check",
            modelValue: checked.value,
            "onUpdate:modelValue": (next: boolean | undefined) => {
              checked.value = next ?? false
            },
            onChange: raw,
          }),
          h(Slider, {
            testId: "slider",
            value: range.value,
            "onUpdate:value": (next: number | [number, number]) => {
              if (Array.isArray(next)) range.value = next
            },
          }),
          h(Input, {
            testId: "input",
            modelValue: value.value,
            "onUpdate:modelValue": (next: string | undefined) => {
              value.value = next ?? ""
            },
          }),
        ]),
    ),
  )
  try {
    root.findByTestId("check").trigger("change", { value: "true" })
    root.findByTestId("slider").trigger("change", { value: "[30,70]" })
    root.findByTestId("input").trigger("change", { value: '"updated"' })
    await root.flush()
    expect(checked.value).toBe(true)
    expect(range.value).toEqual([30, 70])
    expect(value.value).toBe("updated")
    expect(root.findByTestId("check").prop("checked")).toBe(true)
    expect(root.findByTestId("slider").prop("value")).toEqual([30, 70])
    expect(root.findByTestId("input").prop("modelValue")).toBeUndefined()
    expect(raw).toHaveBeenCalledOnce()
  } finally {
    root.unmount()
  }
})

it("normalizes native boolean presence attributes while preserving empty text values", () => {
  const root = mountGpui(
    defineComponent(() => () => h("kit-input", { testId: "input", disabled: "", value: "" })),
  )
  try {
    expect(root.findByTestId("input").prop("disabled")).toBe(true)
    expect(root.findByTestId("input").prop("value")).toBe("")
  } finally {
    root.unmount()
  }
})

it("accepts kebab-case Kit props and value callbacks from Vue templates", () => {
  const changed = vi.fn<(value: boolean) => void>()
  const root = mountGpui(
    defineComponent(
      () => () => h(Checkbox, { testId: "check", "model-value": true, "on-value-change": changed }),
    ),
  )
  try {
    expect(root.findByTestId("check").prop("checked")).toBe(true)
    root.findByTestId("check").trigger("change", { value: "false" })
    expect(changed).toHaveBeenCalledWith(false)
  } finally {
    root.unmount()
  }
})
