# React

`@gpui-native/react` is a React 19.2 custom renderer. It uses React's reconciler
and the same Rust engine, native elements, mutation protocol, text pipeline,
and WebAssembly backend as Vue. It does not load Vue or React DOM.

## Run the examples

From a checkout:

```bash
bun install
bun run build:native
bun run build:adapters
bun --filter '@gpui-react/example-*' build
bun --filter @gpui-react/example-counter start
bun --filter @gpui-react/example-showcase start
bun --filter @gpui-react/example-multiple-windows start
```

The counter demonstrates state, events, controlled input and window size. The
showcase demonstrates Tooltip, Select, Combobox, code, Markdown, canvas, native
motion, text search and virtual lists. Multiple windows demonstrates independent
React roots and window controls. All three also appear in the Pages gallery.

## JSX and components

Use React 19.2 with `react-reconciler` 0.33 (pinned by the adapter). Configure:

```json
{
  "compilerOptions": {
    "jsx": "react-jsx",
    "jsxImportSource": "@gpui-native/react"
  }
}
```

```tsx
import { useState } from "react"
import { render, Button, Column, Text, TextInput } from "@gpui-native/react"

function App() {
  const [name, setName] = useState("world")
  return (
    <Column style={{ padding: 24, gap: 12 }}>
      <Text>Hello, {name}</Text>
      <TextInput value={name} onChange={(event) => setName(event.value ?? "")} />
      <Button onPress={() => setName("world")} style={{ padding: 10 }}>
        Reset
      </Button>
    </Column>
  )
}

render(<App />, { title: "React + GPUI", width: 720, height: 500 })
```

Supported intrinsic tags are `div`, `text`, `img`, `svg`, `canvas`, `input`,
`textarea`, `anchored`, `code`, `diff`, `markdown`, and `virtual-list`. Each also
has a typed component with a [short, framework-neutral name](components.md).
`Row`, `Column`, `ScrollView`, and the unstyled `Button` build on these primitives.
Use `onPress` for button activation by pointer, Enter, or Space. Styles and events are GPUI's types, not DOM CSS
or synthetic browser events. Inputs use `value` and `onChange`; there is no Vue
`v-model`. Native `<code>` remains bare: the application supplies its container
styling and language header.

React owns hooks, context, effects, fragments, keyed reconciliation, refs,
StrictMode, Suspense and error boundaries. Host refs expose `GpuiPublicInstance`
with its native `id`. Render-phase instances remain local until commit, including
instances discarded by Suspense. Mutations and prop removals use the shared
atomic `applyBatch` protocol. `createGpuiRenderer` provides a low-level root with
`render`, `flush`, `destroy`, and same-root `createPortal`.

## Vue capability mapping

| Vue API                                 | React API                                                              |
| --------------------------------------- | ---------------------------------------------------------------------- |
| `render(App, options)`                  | `render(<App />, options)`                                             |
| `createWindow(App, options)`            | `createWindow(<App />, options)`                                       |
| `ref`, `computed`, lifecycle hooks      | React `useState`, `useMemo`, `useEffect`                               |
| `useGpui`, `useGpuiRequired`            | Same names, React context                                              |
| `useElementRef`                         | React ref object (`.current`)                                          |
| `useWindowSize`, `useWindowInsets`      | Same names, return values directly                                     |
| `useGpuiWindow`                         | Same native window, selection, scrolling and debug controls            |
| `useGpuiTimeline`, `useGpuiAudioFrames` | Same controls, memoized per renderer                                   |
| `motion.View`, `stagger`                | Same declarative native motion                                         |
| `useTextSearch`                         | Same search behavior; spread `search.props` directly                   |
| Tooltip / Select / Combobox parts       | React components, controlled or uncontrolled state                     |
| Default slots                           | `children`; item state and filtered lists also accept render functions |
| `asChild`                               | One React element, composed events and refs                            |
| `mountGpui`, `createTestRoot`           | React elements and React `act`                                         |
| Automation API                          | Shared `@gpui-native/react/automation` entry                           |

Select uses `value`, `defaultValue`, `onValueChange`, `open`, `defaultOpen`, and
`onOpenChange`. Combobox adds controlled input, multiple selection, filtering,
and optional automatic highlighting. Tooltip supports provider delays, focus,
hover, Escape and outside dismissal. GPUI owns popup placement and collision
handling. React and Vue may run in separate windows; a native renderer permits
only one live framework root.

## Browser

Import `initGpuiWeb` and `render` from `@gpui-native/react/web`, initialize the
Wasm bridge, then render an element. Resolve `@gpui-native/wasm` to the generated
`web/pkg/gpui_core.js`, as the Pages build does. The browser entry excludes
native addon and GPU test loading. It uses the existing single-threaded GPUI Web
backend and requires no shared-memory headers.

## Testing

```tsx
import { act, mountGpui } from "@gpui-native/react/testing"

const root = mountGpui(<App />)
await act(async () => {
  root.findByTestId("name").trigger("change", { value: "React" })
})
await root.flush()
root.unmount()
```

`mountGpui` uses the memory renderer. `createTestRoot` uses the GPU test backend
on macOS and Windows and exposes real input simulation, painted text, bounds,
accessibility and screenshots. `bun run test:native` includes a React GPU smoke
test. Linux continues to use the production renderer without screenshot tests.

Run `bun run check`, `bun test`, `cargo test --workspace --all-features`,
`bun run build:native`, `bun run test:native`, and `bun run build:pages` before
handoff. `bun run test` runs both framework suites and the Rust tests.
