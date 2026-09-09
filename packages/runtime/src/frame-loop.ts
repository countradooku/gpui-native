import { subscribeRendererEvent } from "./events.js"
import type { NativeRenderer } from "./native.js"
import { reportRuntimeError } from "./runtime-errors.js"
export interface FrameLoop {
  stop(): void
}
const DEFAULT_FRAME_MS = 8
const DEFAULT_LIVENESS_MS = 100
// Node and Bun clamp timers to a signed 32-bit millisecond delay. One dormant
// interval keeps the JavaScript runtime alive without polling the native UI.
const EVENT_LOOP_KEEP_ALIVE_MS = 2_147_483_647

export function startFrameLoop(
  renderer: NativeRenderer,
  options: {
    frameMs?: number
    livenessMs?: number
    keepAlive?: boolean
    onTerminated?: () => void
  } = {},
): FrameLoop {
  if (renderer.requiresTick?.() !== true || renderer.tick === undefined) {
    if (options.keepAlive !== true) return { stop() {} }
    if (renderer.supportsWindowEvents?.() === true) {
      let stopped = false
      // N-API event callbacks are deliberately unreferenced, and a native UI
      // thread alone does not retain the JavaScript process. Hold one dormant
      // timer until the authoritative native close event arrives.
      const keepAliveTimer = setInterval(() => undefined, EVENT_LOOP_KEEP_ALIVE_MS)
      let unsubscribe: (() => void) | undefined
      const stop = (): void => {
        if (stopped) return
        stopped = true
        clearInterval(keepAliveTimer)
        unsubscribe?.()
      }
      unsubscribe = subscribeRendererEvent(renderer, "windowClose", () => {
        stop()
        options.onTerminated?.()
      })
      return { stop }
    }

    let timer: ReturnType<typeof setTimeout> | undefined
    let stopped = false
    const stop = (): void => {
      stopped = true
      if (timer !== undefined) clearTimeout(timer)
      timer = undefined
    }
    const poll = (): void => {
      if (stopped) return
      if (renderer.isInitialized?.() === false) {
        stop()
        options.onTerminated?.()
        return
      }
      timer = setTimeout(poll, options.livenessMs ?? DEFAULT_LIVENESS_MS)
    }
    poll()
    return { stop }
  }

  const frameMs = options.frameMs ?? DEFAULT_FRAME_MS
  let timer: ReturnType<typeof setTimeout> | undefined
  let stopped = false
  const stop = (): void => {
    stopped = true
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
  }
  const loop = (): void => {
    if (stopped) return
    const started = performance.now()
    try {
      if (renderer.tick?.() === false) {
        stop()
        options.onTerminated?.()
        return
      }
    } catch (error) {
      reportRuntimeError(renderer, error)
    }
    timer = setTimeout(loop, Math.max(0, frameMs - (performance.now() - started)))
  }
  loop()
  return { stop }
}
