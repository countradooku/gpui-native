import type { NativeRenderer } from "./native.js"
import type { AudioBufferState } from "./types.js"

const EMPTY_STATE: AudioBufferState = {
  sampleRate: 48_000,
  channels: 2,
  capacityFrames: 0,
  queuedFrames: 0,
  droppedFrames: 0,
}

/** High-throughput decoded f32 PCM bridge into the native renderer. */
export class GpuiAudioFrames {
  constructor(private readonly renderer: NativeRenderer) {}

  configure(sampleRate: number, channels: number, capacityFrames?: number): AudioBufferState {
    return this.renderer.configureAudio?.(sampleRate, channels, capacityFrames) ?? this.state()
  }

  enqueue(interleavedSamples: Float32Array): AudioBufferState {
    if (!(interleavedSamples instanceof Float32Array)) {
      throw new TypeError("audio enqueue expects a Float32Array")
    }
    return this.renderer.enqueueAudioFrames?.(interleavedSamples) ?? this.state()
  }

  dequeue(maxFrames: number): Float32Array {
    return this.renderer.dequeueAudioFrames?.(maxFrames) ?? new Float32Array()
  }

  clear(): AudioBufferState {
    return this.renderer.clearAudioFrames?.() ?? this.state()
  }

  state(): AudioBufferState {
    return this.renderer.getAudioBufferState?.() ?? { ...EMPTY_STATE }
  }
}

export function createAudioFrames(renderer: NativeRenderer): GpuiAudioFrames {
  return new GpuiAudioFrames(renderer)
}
