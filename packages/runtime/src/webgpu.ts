/// <reference types="@webgpu/types" preserve="true" />

import type { NativeRenderer } from "./native.js"

export interface GPUCanvasOptions {
  renderer: NativeRenderer
  width?: number
  height?: number
  /** Maximum outstanding readbacks. A busy presenter drops new frames. Default 2. */
  maxFramesInFlight?: number
}

export interface GPUCanvasStats {
  submitted: number
  presented: number
  dropped: number
  failed: number
  inFlight: number
  bytesPresented: number
  lastPresentMs: number
  /** CPU readback is used on every platform; this is not zero-copy. */
  transport: "async-readback"
}

type Slot = { buffer: GPUBuffer | undefined; size: number; busy: boolean }
const COPY_SRC = 1
const COPY_DST = 8
const MAP_READ = 1
const RENDER_ATTACHMENT = 16
const MAX_BYTES = 64 * 1024 * 1024

function dimension(value: number): number {
  if (!Number.isInteger(value) || value < 1 || value > 8192) {
    throw new RangeError("GPU canvas dimensions must be integers between 1 and 8192")
  }
  return value
}

/**
 * A WebGPU texture target composited by GPUI's canvas primitive. It supplies the
 * canvas methods used by WebGPU renderers, but is not a DOM HTMLCanvasElement.
 * Call present() after submitting rendering commands. GPU objects are real
 * browser/Dawn objects; GPUI does not wrap their API or suppress validation.
 */
export class GPUCanvas extends EventTarget {
  readonly id: number
  readonly style: Record<string, string> = {}
  readonly #renderer: Required<
    Pick<NativeRenderer, "createCanvasSource" | "presentCanvasFrame" | "destroyCanvasSource">
  >
  readonly #slots: Slot[]
  readonly #context: GPUCanvasContext
  #width: number
  #height: number
  #configuration: GPUCanvasConfiguration | undefined
  #texture: GPUTexture | undefined
  readonly #watchedDevices = new WeakSet<GPUDevice>()
  #generation = 0
  #sequence = 0
  #publishedSequence = 0
  #destroyed = false
  #stats: GPUCanvasStats = {
    submitted: 0,
    presented: 0,
    dropped: 0,
    failed: 0,
    inFlight: 0,
    bytesPresented: 0,
    lastPresentMs: 0,
    transport: "async-readback",
  }

  constructor(options: GPUCanvasOptions) {
    super()
    const { renderer } = options
    if (
      !renderer.createCanvasSource ||
      !renderer.presentCanvasFrame ||
      !renderer.destroyCanvasSource
    ) {
      throw new Error("This GPUI renderer does not support GPU canvas frame transport")
    }
    this.#width = dimension(options.width ?? 300)
    this.#height = dimension(options.height ?? 150)
    this.#validateSize(this.#width, this.#height)
    const count = options.maxFramesInFlight ?? 2
    if (!Number.isInteger(count) || count < 1 || count > 3)
      throw new RangeError("maxFramesInFlight must be 1, 2, or 3")
    this.#renderer = {
      createCanvasSource: renderer.createCanvasSource.bind(renderer),
      presentCanvasFrame: renderer.presentCanvasFrame.bind(renderer),
      destroyCanvasSource: renderer.destroyCanvasSource.bind(renderer),
    }
    this.#slots = Array.from({ length: count }, () => ({ buffer: undefined, size: 0, busy: false }))
    this.#context = {
      canvas: this,
      configure: (configuration: GPUCanvasConfiguration) => this.configure(configuration),
      unconfigure: () => this.unconfigure(),
      getConfiguration: () =>
        this.#configuration
          ? { ...this.#configuration, viewFormats: [...(this.#configuration.viewFormats ?? [])] }
          : null,
      getCurrentTexture: () => this.#currentTexture(),
    } as unknown as GPUCanvasContext
    this.id = this.#renderer.createCanvasSource()
  }

  get width(): number {
    return this.#width
  }
  set width(value: number) {
    this.resize(value, this.#height)
  }
  get height(): number {
    return this.#height
  }
  set height(value: number) {
    this.resize(this.#width, value)
  }
  get clientWidth(): number {
    return this.width
  }
  get clientHeight(): number {
    return this.height
  }
  get destroyed(): boolean {
    return this.#destroyed
  }
  get stats(): Readonly<GPUCanvasStats> {
    return { ...this.#stats }
  }

  getContext(type: "webgpu"): GPUCanvasContext
  getContext(type: string): GPUCanvasContext | null
  getContext(type: string): GPUCanvasContext | null {
    return type === "webgpu" ? this.#context : null
  }

  configure(configuration: GPUCanvasConfiguration): void {
    this.#assertAlive()
    if (
      configuration.alphaMode &&
      configuration.alphaMode !== "opaque" &&
      configuration.alphaMode !== "premultiplied"
    ) {
      throw new TypeError("GPU canvas alphaMode must be opaque or premultiplied")
    }
    if (configuration.format !== "rgba8unorm" && configuration.format !== "bgra8unorm") {
      throw new TypeError("GPUI presentation supports rgba8unorm and bgra8unorm")
    }
    if (
      (configuration.colorSpace && configuration.colorSpace !== "srgb") ||
      (configuration.toneMapping?.mode && configuration.toneMapping.mode !== "standard")
    ) {
      throw new TypeError("GPUI presentation currently supports standard sRGB only")
    }
    if (
      this.width > configuration.device.limits.maxTextureDimension2D ||
      this.height > configuration.device.limits.maxTextureDimension2D
    ) {
      throw new RangeError("GPU canvas dimensions exceed the device limit")
    }
    this.#releaseGPU()
    this.#configuration = { ...configuration, viewFormats: [...(configuration.viewFormats ?? [])] }
    if (!this.#watchedDevices.has(configuration.device)) {
      this.#watchedDevices.add(configuration.device)
      const weak = new WeakRef(this)
      const device = configuration.device
      void device.lost.then(() => {
        const canvas = weak.deref()
        if (canvas && !canvas.#destroyed && canvas.#configuration?.device === device) {
          canvas.unconfigure()
          canvas.dispatchEvent(new Event("contextlost"))
        }
      })
    }
  }

  unconfigure(): void {
    this.#releaseGPU()
    this.#configuration = undefined
  }

  resize(width: number, height: number): void {
    this.#assertAlive()
    dimension(width)
    dimension(height)
    this.#validateSize(width, height)
    if (width === this.#width && height === this.#height) return
    const limit = this.#configuration?.device.limits.maxTextureDimension2D ?? 8192
    if (width > limit || height > limit)
      throw new RangeError("GPU canvas dimensions exceed the device limit")
    // Keep device-loss tracking alive across resize by reconfiguring below.
    const configuration = this.#configuration
    this.#releaseGPU()
    this.#width = width
    this.#height = height
    if (configuration) this.configure(configuration)
  }

  /** Resolve false when backpressure, resizing, or disposal makes a frame obsolete. */
  async present(): Promise<boolean> {
    this.#assertAlive()
    const configuration = this.#configuration
    if (!configuration) throw new Error("Configure the WebGPU canvas before presenting")
    const slot = this.#slots.find((candidate) => !candidate.busy)
    if (!slot) {
      this.#stats.dropped++
      return false
    }
    const width = this.width,
      height = this.height
    const stride = Math.ceil((width * 4) / 256) * 256
    const size = stride * height
    const generation = this.#generation
    const sequence = ++this.#sequence
    const started = performance.now()
    slot.busy = true
    this.#stats.inFlight++
    let buffer: GPUBuffer | undefined
    try {
      if (!slot.buffer || slot.size !== size) {
        slot.buffer?.destroy()
        slot.buffer = configuration.device.createBuffer({
          label: "GPUI canvas readback",
          size,
          usage: COPY_DST | MAP_READ,
        })
        slot.size = size
      }
      buffer = slot.buffer
      const encoder = configuration.device.createCommandEncoder({
        label: "GPUI canvas presentation",
      })
      encoder.copyTextureToBuffer(
        { texture: this.#currentTexture() },
        { buffer, bytesPerRow: stride, rowsPerImage: height },
        { width, height },
      )
      configuration.device.queue.submit([encoder.finish()])
      this.#stats.submitted++
      await buffer.mapAsync(MAP_READ)
      if (
        generation !== this.#generation ||
        this.#destroyed ||
        sequence <= this.#publishedSequence
      ) {
        this.#stats.dropped++
        return false
      }
      const pixels = new Uint8Array(buffer.getMappedRange())
      // The native/Wasm boundary copies synchronously before this buffer unmaps.
      this.#renderer.presentCanvasFrame(
        this.id,
        width,
        height,
        stride,
        pixels,
        configuration.format === "bgra8unorm",
        (configuration.alphaMode ?? "opaque") === "opaque",
      )
      this.#publishedSequence = sequence
      this.#stats.presented++
      this.#stats.bytesPresented += width * height * 4
      this.#stats.lastPresentMs = performance.now() - started
      return true
    } catch (error) {
      if (generation !== this.#generation || this.#destroyed) {
        this.#stats.dropped++
        return false
      }
      this.#stats.failed++
      // A failed mapping cannot safely return its buffer to the pool.
      buffer?.destroy()
      slot.buffer = undefined
      throw error
    } finally {
      if (buffer?.mapState === "mapped") buffer.unmap()
      slot.busy = false
      this.#stats.inFlight--
    }
  }

  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.unconfigure()
    this.#renderer.destroyCanvasSource(this.id)
  }

  #assertAlive(): void {
    if (this.#destroyed) throw new Error("GPU canvas has been destroyed")
  }
  #validateSize(width: number, height: number): void {
    if (Math.ceil((width * 4) / 256) * 256 * height > MAX_BYTES)
      throw new RangeError("GPU canvas frame exceeds 64 MiB")
  }
  #releaseGPU(): void {
    this.#generation++
    this.#texture?.destroy()
    this.#texture = undefined
    for (const slot of this.#slots) {
      slot.buffer?.destroy()
      slot.buffer = undefined
      slot.size = 0
    }
  }
  #currentTexture(): GPUTexture {
    this.#assertAlive()
    const configuration = this.#configuration
    if (!configuration) throw new Error("Configure the WebGPU canvas before acquiring a texture")
    return (this.#texture ??= configuration.device.createTexture({
      label: "GPUI canvas target",
      size: { width: this.width, height: this.height },
      format: configuration.format,
      usage: (configuration.usage ?? RENDER_ATTACHMENT) | COPY_SRC,
      viewFormats: configuration.viewFormats ?? [],
    }))
  }
}

export function createGPUCanvas(options: GPUCanvasOptions): GPUCanvas {
  return new GPUCanvas(options)
}
