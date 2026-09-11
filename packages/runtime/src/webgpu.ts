/// <reference types="@webgpu/types" preserve="true" />

import { nativeDevice, nativeTexture } from "./gpu-interop.js"
import type { NativeRenderer } from "./native.js"

export interface GPUCanvasOptions {
  /** Direct GPU snapshots are the default. Readback must be selected explicitly. */
  presentation?: "direct" | "async-readback"
  renderer: NativeRenderer
  width?: number
  height?: number
  /** Maximum outstanding presentations. A busy presenter drops new frames. Default 2. */
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
  /** Actual transport; GPU copy never maps pixel bytes for presentation. */
  transport: "gpu-copy" | "async-readback"
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
 * browser objects or the shared Rust wgpu binding; validation failures remain observable.
 */
export class GPUCanvas extends EventTarget {
  readonly id: number
  readonly style: Record<string, string> = {}
  readonly #renderer: Required<
    Pick<NativeRenderer, "createCanvasSource" | "presentCanvasFrame" | "destroyCanvasSource">
  >
  readonly #host: NativeRenderer
  #webHandle: number | undefined
  #ownsDevice = false
  readonly #direct?: NativeRenderer["presentCanvasTexture"]
  readonly #reset?: NativeRenderer["resetCanvasSource"]
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
    this.#host = renderer
    if (
      !renderer.createCanvasSource ||
      !renderer.presentCanvasFrame ||
      !renderer.destroyCanvasSource
    ) {
      throw new Error("This GPUI renderer does not support GPU canvas frame transport")
    }
    if ((options.presentation ?? "direct") === "direct") {
      if (!renderer.presentCanvasTexture || renderer.canvasPresentation?.() === "unsupported")
        throw new Error(
          "This renderer has no direct GPU presentation; choose presentation: async-readback explicitly",
        )
      this.#direct = renderer.presentCanvasTexture.bind(renderer)
      this.#stats.transport = "gpu-copy"
    }
    this.#reset = renderer.resetCanvasSource?.bind(renderer)
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

  /** Whether requestDevice() created an owned device that the caller must destroy. */
  get ownsDevice(): boolean {
    return this.#ownsDevice
  }

  /** Acquire the compositor device where required; otherwise create an owned device. */
  async requestDevice(gpu: GPU, descriptor: GPUDeviceDescriptor = {}): Promise<GPUDevice> {
    this.#assertAlive()
    if (this.#direct && this.#host.canvasPresentation?.() === "shared-wgpu") {
      const deadline = performance.now() + 10000
      while (performance.now() < deadline) {
        this.#assertAlive()
        const device = this.#host.canvasGpuDevice?.()
        if (device) {
          for (const feature of descriptor.requiredFeatures ?? [])
            if (!device.features.has(feature))
              throw new Error(`GPUI shared device does not enable ${feature}`)
          for (const [name, required] of Object.entries(descriptor.requiredLimits ?? {})) {
            const actual = (device.limits as unknown as Record<string, number>)[name]
            const satisfied = name.startsWith("min") ? actual! <= required! : actual! >= required!
            if (actual === undefined || !satisfied)
              throw new Error(`GPUI shared device cannot satisfy ${name}=${required}`)
          }
          this.#ownsDevice = false
          return device
        }
        await new Promise((resolve) => setTimeout(resolve, 10))
      }
      throw new Error(
        "GPUI's WebGPU device did not become ready; WebGL renderers require explicit async-readback",
      )
    }
    const adapter = await gpu.requestAdapter({ powerPreference: "high-performance" })
    if (!adapter) throw new Error("No WebGPU adapter is available")
    const device = await adapter.requestDevice(descriptor)
    if (this.#destroyed) {
      device.destroy()
      throw new Error("GPU canvas destroyed while creating its device")
    }
    this.#ownsDevice = true
    return device
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
    if (
      this.#direct &&
      !(nativeDevice in configuration.device) &&
      configuration.device !== this.#host.canvasGpuDevice?.()
    )
      throw new Error(
        "Direct native presentation requires the GPUI wgpu device; browser or foreign devices require explicit async-readback",
      )
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
    if (this.#direct) return this.#presentDirect(configuration)
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

  async #presentDirect(configuration: GPUCanvasConfiguration): Promise<boolean> {
    if (this.#stats.inFlight >= this.#slots.length) {
      this.#stats.dropped++
      return false
    }
    const texture = this.#currentTexture()
    const generation = this.#generation
    const started = performance.now()
    this.#stats.inFlight++
    this.#stats.submitted++
    try {
      const result = await this.#direct!(
        this.id,
        nativeDevice in configuration.device
          ? Reflect.get(configuration.device, nativeDevice)
          : configuration.device,
        this.#webHandle ?? (Reflect.get(texture, nativeTexture) as number),
        (configuration.alphaMode ?? "opaque") === "opaque",
      )
      if (!result || generation !== this.#generation || this.#destroyed) {
        this.#stats.dropped++
        return false
      }
      this.#stats.presented++
      this.#stats.lastPresentMs = performance.now() - started
      return true
    } catch (error) {
      if (generation !== this.#generation || this.#destroyed) {
        this.#stats.dropped++
        return false
      }
      this.#stats.failed++
      throw error
    } finally {
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
    this.#reset?.(this.id)
    this.#texture?.destroy()
    if (this.#webHandle !== undefined) this.#host.releaseCanvasTexture?.(this.#webHandle)
    this.#webHandle = undefined
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
    if (this.#texture) return this.#texture
    const descriptor: GPUTextureDescriptor = {
      label: "GPUI canvas target",
      size: { width: this.width, height: this.height },
      format: configuration.format,
      usage: (configuration.usage ?? RENDER_ATTACHMENT) | COPY_SRC,
      viewFormats: configuration.viewFormats ?? [],
    }
    if (this.#direct && this.#host.createCanvasTexture) {
      const target = this.#host.createCanvasTexture(descriptor)
      this.#webHandle = target.handle
      this.#texture = target.texture
    } else {
      this.#texture = configuration.device.createTexture(descriptor)
    }
    return this.#texture
  }
}

export function createGPUCanvas(options: GPUCanvasOptions): GPUCanvas {
  return new GPUCanvas(options)
}
