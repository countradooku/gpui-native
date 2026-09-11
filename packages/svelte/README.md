# @gpui-native/svelte

Svelte 5 runes for GPUI native windows and the shared WebAssembly renderer.
Includes typed native components, bindings, headless controls, motion, window
APIs, text search, GPU canvas lifecycle, testing, and a Vite compiler plugin.

Use `gpuiSvelte()` from `@gpui-native/svelte/vite` to compile `.svelte` components
and `.svelte.ts` modules. The adapter pins Svelte 5.57.0 and bundles an isolated
copy of its client runtime. GPUI owns layout, input, text and painting.

```svelte
<script>
  import { Button, Column, Text } from '@gpui-native/svelte'
  let count = $state(0)
</script>
<Column style={{ padding: 24 }}>
  <Text>{count}</Text>
  <Button onPress={() => count++}>Increment</Button>
</Column>
```

See [the complete guide](https://github.com/countradooku/gpui-native/blob/main/docs/svelte.md)
for native/browser setup, supported syntax, examples, tests and platform limits.
This is a native Svelte target; DOM libraries, CSS stylesheets, SSR and hydration
are not supported. The package follows GPUI Native's early 0.2.x release status.
