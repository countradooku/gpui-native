import type { GpuiIntrinsicElements } from "./types.js"

declare global {
  namespace JSX {
    interface IntrinsicElements extends GpuiIntrinsicElements {}
  }
}

export {}
