import { expect, it } from "vitest"

import { mountGpui } from "../dist/testing.js"
import { fixture } from "./helpers.js"
it("binds Kit checked, value, and open properties to Svelte state", async () => {
  const App = await fixture(
    `<script>import {Checkbox,Input,Dialog} from '@gpui-native/svelte/kit';import {Text} from '@gpui-native/svelte';let checked=$state(false);let value=$state('first');let open=$state(true)</script><Checkbox testId="check" bind:checked/><Input testId="input" bind:value/><Dialog testId="dialog" bind:open><Text>Body</Text></Dialog><Text testId="state">{checked}:{value}:{open}</Text>`,
  )
  const root = mountGpui(App)
  try {
    await root.flush()
    root.findByTestId("check").trigger("change", { value: "true" })
    root.findByTestId("input").trigger("change", { value: '"updated"' })
    root.findByTestId("dialog").trigger("change", { value: "false" })
    await root.flush()
    expect(root.findByTestId("state").text).toBe("true:updated:false")
    expect(root.findByTestId("check").prop("checked")).toBe(true)
    expect(root.findByTestId("input").prop("value")).toBe("updated")
    expect(root.findByTestId("dialog").prop("open")).toBe(false)
  } finally {
    root.unmount()
  }
})

it("binds compiled Kit native tags through the same model bridge", async () => {
  const App = await fixture(
    `<script>let checked=$state(false)</script><kit-checkbox testId="raw" bind:checked/><text testId="state">{checked}</text>`,
  )
  const root = mountGpui(App)
  try {
    await root.flush()
    root.findByTestId("raw").trigger("change", { value: "true" })
    await root.flush()
    expect(root.findByTestId("state").text).toBe("true")
  } finally {
    root.unmount()
  }
})
