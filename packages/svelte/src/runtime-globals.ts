// Injected only into the bundled Svelte runtime. Never replaces browser globals.
export {
  nativeDocument as document,
  nativeWindow as window,
  nativeNavigator as navigator,
  HostNode as Node,
  HostElement as Element,
  HostText as Text,
  HostComment as Comment,
} from "./host.js"
