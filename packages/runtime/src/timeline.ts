import type { NativeRenderer } from "./native.js"
import type { TimelineState } from "./types.js"

/** Imperative controls for the renderer's native animation timeline. */
export class GpuiTimeline {
  constructor(private readonly renderer: NativeRenderer) {}

  state(): TimelineState {
    return (
      this.renderer.timelineGetState?.() ?? {
        currentTimeMs: 0,
        playbackRate: 1,
        playing: true,
      }
    )
  }

  play(): TimelineState {
    this.renderer.timelinePlay?.()
    return this.state()
  }

  pause(): TimelineState {
    this.renderer.timelinePause?.()
    return this.state()
  }

  seek(currentTimeMs: number): TimelineState {
    this.renderer.timelineSeek?.(currentTimeMs)
    return this.state()
  }

  setPlaybackRate(playbackRate: number): TimelineState {
    this.renderer.timelineSetPlaybackRate?.(playbackRate)
    return this.state()
  }
}

export function createTimeline(renderer: NativeRenderer): GpuiTimeline {
  return new GpuiTimeline(renderer)
}
