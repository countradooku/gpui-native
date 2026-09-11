/// <reference types="@webgpu/types" preserve="true" />

import type { NativeWgpuAdapter, NativeWgpuDevice } from "@gpui-native/core"

import { nativeDevice, nativeTexture } from "./gpu-interop.js"
export { nativeDevice, nativeTexture } from "./gpu-interop.js"
export class GPUValidationError extends Error {}
export class GPUOutOfMemoryError extends Error {}
export class GPUInternalError extends Error {}
export class GPUPipelineError extends Error {
  readonly reason: GPUPipelineErrorReason
  constructor(message: string) {
    super(message)
    this.reason = /^(internal|out-of-memory):/.test(message) ? "internal" : "validation"
  }
}
export class GPUUncapturedErrorEvent extends Event {
  readonly error: GPUError
  constructor(type: string, init: GPUUncapturedErrorEventInit) {
    super(type)
    this.error = init.error
  }
}

const finalizer = new FinalizationRegistry<{ native: NativeWgpuDevice; id: number }>(
  ({ native, id }) => native.release(id),
)

type Descriptor = Record<string, unknown>
type Command = unknown[]

class Resource {
  label: string
  constructor(
    readonly device: Device,
    readonly id: number,
    descriptor: Descriptor = {},
  ) {
    this.label = String(descriptor.label ?? "")
    if (id) finalizer.register(this, { native: device[nativeDevice], id }, this)
  }
}

function serialize(value: unknown, device: Device, references?: Set<Resource>): unknown {
  if (value instanceof Resource) {
    if (value.device !== device) throw new TypeError("GPU resources cannot cross device owners")
    references?.add(value)
    return value.id
  }
  if (Array.isArray(value)) return value.map((v) => serialize(v, device, references))
  if (ArrayBuffer.isView(value)) return Array.from(value as unknown as ArrayLike<number>)
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, value]) => [key, serialize(value, device, references)]),
    )
  }
  return value
}

function bytes(data: AllowSharedBufferSource, offset = 0, size?: number): Uint8Array {
  const view = ArrayBuffer.isView(data)
    ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
    : new Uint8Array(data)
  const elementSize =
    ArrayBuffer.isView(data) && "BYTES_PER_ELEMENT" in data ? Number(data.BYTES_PER_ELEMENT) : 1
  const start = offset * elementSize
  const length = size === undefined ? view.byteLength - start : size * elementSize
  if (
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(length) ||
    start < 0 ||
    length < 0 ||
    start + length > view.byteLength
  ) {
    throw new RangeError("GPU upload range is outside the supplied data")
  }
  return new Uint8Array(view.buffer, view.byteOffset + start, length)
}

class BufferResource extends Resource {
  readonly size: number
  readonly usage: number
  mapState: GPUBufferMapState
  #mode = 2
  #offset = 0
  #length: number
  #generation = 0
  #destroyed = false
  #ranges: { offset: number; data: ArrayBuffer }[] = []
  constructor(device: Device, id: number, descriptor: Descriptor) {
    super(device, id, descriptor)
    this.size = Number(descriptor.size)
    this.usage = Number(descriptor.usage)
    this.#length = this.size
    this.mapState = descriptor.mappedAtCreation ? "mapped" : "unmapped"
  }
  async mapAsync(mode: GPUMapModeFlags, offset = 0, size = this.size - offset): Promise<void> {
    if (this.#destroyed || this.mapState !== "unmapped")
      throw new DOMException("Buffer cannot be mapped", "OperationError")
    if (offset % 8 || size % 4 || offset < 0 || size < 0 || offset + size > this.size)
      throw new RangeError("Invalid buffer map range")
    this.mapState = "pending"
    const generation = ++this.#generation
    try {
      await this.device[nativeDevice].map(this.id, mode, offset, size)
      if (generation !== this.#generation || this.#destroyed)
        throw new DOMException("Mapping cancelled", "AbortError")
      this.#mode = mode
      this.#offset = offset
      this.#length = size
      this.mapState = "mapped"
    } catch (error) {
      if (generation === this.#generation) this.mapState = "unmapped"
      throw error
    }
  }
  getMappedRange(offset = 0, size = this.size - offset): ArrayBuffer {
    if (this.mapState !== "mapped" || this.#destroyed)
      throw new DOMException("Buffer is not mapped", "OperationError")
    if (
      offset % 8 ||
      size % 4 ||
      offset < this.#offset ||
      size < 0 ||
      offset + size > this.#offset + this.#length
    )
      throw new RangeError("Invalid mapped range")
    if (this.#ranges.some((r) => offset < r.offset + r.data.byteLength && r.offset < offset + size))
      throw new DOMException("Mapped ranges overlap", "OperationError")
    const data = this.device[nativeDevice].mappedBytes(this.id, offset, size).slice()
      .buffer as ArrayBuffer
    this.#ranges.push({ offset, data })
    return data
  }
  unmap(): void {
    ++this.#generation
    if (this.#destroyed || this.mapState === "unmapped") return
    for (const range of this.#ranges) {
      if (this.#mode === 2)
        this.device[nativeDevice].writeMapped(this.id, range.offset, new Uint8Array(range.data))
      structuredClone(range.data, { transfer: [range.data] })
    }
    this.#ranges = []
    this.device[nativeDevice].unmap(this.id)
    this.mapState = "unmapped"
  }
  destroy(): void {
    if (this.#destroyed) return
    this.unmap()
    this.#destroyed = true
    this.device.call(() => this.device[nativeDevice].destroyResource(this.id))
  }
}

class Texture extends Resource {
  readonly width: number
  readonly height: number
  readonly depthOrArrayLayers: number
  readonly mipLevelCount: number
  readonly sampleCount: number
  readonly dimension: GPUTextureDimension
  readonly format: GPUTextureFormat
  readonly usage: number
  #destroyed = false
  get [nativeTexture](): number {
    return this.id
  }
  constructor(device: Device, id: number, descriptor: Descriptor) {
    super(device, id, descriptor)
    const size = descriptor.size as GPUExtent3DDict | number[]
    this.width = Array.isArray(size) ? size[0]! : size.width
    this.height = (Array.isArray(size) ? size[1] : size.height) ?? 1
    this.depthOrArrayLayers = (Array.isArray(size) ? size[2] : size.depthOrArrayLayers) ?? 1
    this.mipLevelCount = Number(descriptor.mipLevelCount ?? 1)
    this.sampleCount = Number(descriptor.sampleCount ?? 1)
    this.dimension = (descriptor.dimension ?? "2d") as GPUTextureDimension
    this.format = descriptor.format as GPUTextureFormat
    this.usage = Number(descriptor.usage)
  }
  createView(descriptor: GPUTextureViewDescriptor = {}): Resource {
    return this.device.resource("view", { ...descriptor, texture: this })
  }
  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.device.call(() => this.device[nativeDevice].destroyResource(this.id))
  }
}

class Pipeline extends Resource {
  getBindGroupLayout(index: number): Resource {
    return this.device.resource("pipelineBindGroupLayout", { pipeline: this, index })
  }
}
class Shader extends Resource {
  readonly #code: string
  constructor(device: Device, id: number, descriptor: Descriptor) {
    super(device, id, descriptor)
    this.#code = String(descriptor.code ?? "")
  }
  async getCompilationInfo(): Promise<GPUCompilationInfo> {
    if (!this.id)
      return {
        messages: [
          {
            message: "Shader creation failed; inspect the validation error",
            type: "error",
            lineNum: 0,
            linePos: 0,
            offset: 0,
            length: 0,
          },
        ],
      } as unknown as GPUCompilationInfo
    const info = (await this.device[nativeDevice].compilationInfo(this.id)) as GPUCompilationInfo
    // wgpu reports UTF-8 byte offsets; WebGPU's JavaScript API uses UTF-16 units.
    const source = new TextEncoder().encode(this.#code)
    const decoder = new TextDecoder()
    return {
      ...info,
      messages: info.messages.map((message) => {
        const before = decoder.decode(source.subarray(0, message.offset))
        return {
          ...message,
          offset: before.length,
          length: decoder.decode(source.subarray(message.offset, message.offset + message.length))
            .length,
          linePos: message.lineNum ? before.length - before.lastIndexOf("\n") : 0,
        }
      }),
    }
  }
}
class QuerySet extends Resource {
  readonly type: GPUQueryType
  readonly count: number
  #destroyed = false
  constructor(device: Device, id: number, descriptor: Descriptor) {
    super(device, id, descriptor)
    this.type = descriptor.type as GPUQueryType
    this.count = Number(descriptor.count)
  }
  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.device.call(() => this.device[nativeDevice].destroyResource(this.id))
  }
}

class CommandEncoder {
  readonly commands: Command[] = []
  readonly references = new Set<Resource>()
  #finished = false
  #passOpen = false
  label: string
  constructor(
    readonly device: Device,
    descriptor: GPUCommandEncoderDescriptor = {},
  ) {
    this.label = descriptor.label ?? ""
  }
  record(op: string, ...args: unknown[]): void {
    if (this.#finished || this.#passOpen)
      throw new DOMException("Command encoder is locked or finished", "InvalidStateError")
    this.commands.push([op, ...args])
    // Snapshot descriptor values at recording time, retaining GPU objects.
    this.commands[this.commands.length - 1] = serialize(
      this.commands[this.commands.length - 1],
      this.device,
      this.references,
    ) as Command
  }
  beginRenderPass(descriptor: GPURenderPassDescriptor): RenderPass {
    return this.begin("renderPass", descriptor) as RenderPass
  }
  beginComputePass(descriptor: GPUComputePassDescriptor = {}): ComputePass {
    return this.begin("computePass", descriptor) as ComputePass
  }
  private begin(kind: string, descriptor: object): RenderPass | ComputePass {
    this.record(kind, descriptor, [])
    this.#passOpen = true
    const commands = this.commands[this.commands.length - 1]![2] as Command[]
    return kind === "renderPass" ? new RenderPass(this, commands) : new ComputePass(this, commands)
  }
  endPass(): void {
    this.#passOpen = false
  }
  copyBufferToBuffer(a: Resource, ao: number, b: Resource, bo: number, size: number): void {
    this.record("copyBufferToBuffer", a, ao, b, bo, size)
  }
  copyTextureToBuffer(
    a: GPUTexelCopyTextureInfo,
    b: GPUTexelCopyBufferInfo,
    size: GPUExtent3D,
  ): void {
    this.record("copyTextureToBuffer", a, b, size)
  }
  copyBufferToTexture(
    a: GPUTexelCopyBufferInfo,
    b: GPUTexelCopyTextureInfo,
    size: GPUExtent3D,
  ): void {
    this.record("copyBufferToTexture", a, b, size)
  }
  copyTextureToTexture(
    a: GPUTexelCopyTextureInfo,
    b: GPUTexelCopyTextureInfo,
    size: GPUExtent3D,
  ): void {
    this.record("copyTextureToTexture", a, b, size)
  }
  clearBuffer(buffer: Resource, offset = 0, size?: number): void {
    this.record("clearBuffer", buffer, offset, size ?? null)
  }
  resolveQuerySet(
    query: Resource,
    first: number,
    count: number,
    buffer: Resource,
    offset: number,
  ): void {
    this.record("resolveQuerySet", query, first, count, buffer, offset)
  }
  insertDebugMarker(label: string): void {
    this.record("insertDebugMarker", label)
  }
  pushDebugGroup(label: string): void {
    this.record("pushDebugGroup", label)
  }
  popDebugGroup(): void {
    this.record("popDebugGroup")
  }
  finish(descriptor: GPUCommandBufferDescriptor = {}): Resource {
    if (this.#finished || this.#passOpen)
      throw new DOMException("Command encoder is locked or finished", "InvalidStateError")
    this.#finished = true
    const id =
      this.device.call(() =>
        this.device[nativeDevice].encode({ label: this.label, commands: this.commands }),
      ) ?? 0
    this.references.clear()
    this.commands.length = 0
    return new Resource(this.device, id, descriptor as Descriptor)
  }
}

class Pass {
  label = ""
  #ended = false
  constructor(
    readonly encoder: CommandEncoder,
    readonly commands: Command[],
  ) {}
  record(op: string, ...args: unknown[]): void {
    if (this.#ended) throw new DOMException("GPU pass already ended", "InvalidStateError")
    this.commands.push(
      serialize([op, ...args], this.encoder.device, this.encoder.references) as Command,
    )
  }
  setPipeline(pipeline: Resource): void {
    this.record("setPipeline", pipeline)
  }
  setBindGroup(
    index: number,
    group: Resource | null,
    offsets: Iterable<number> = [],
    start = 0,
    length?: number,
  ): void {
    const values = Array.from(offsets)
    this.record(
      "setBindGroup",
      index,
      group,
      values.slice(start, length === undefined ? undefined : start + length),
    )
  }
  insertDebugMarker(label: string): void {
    this.record("insertDebugMarker", label)
  }
  pushDebugGroup(label: string): void {
    this.record("pushDebugGroup", label)
  }
  popDebugGroup(): void {
    this.record("popDebugGroup")
  }
  end(): void {
    if (this.#ended) throw new DOMException("GPU pass already ended", "InvalidStateError")
    this.#ended = true
    this.encoder.endPass()
  }
}
class RenderPass extends Pass {
  setVertexBuffer(slot: number, buffer: Resource, offset = 0, size?: number): void {
    this.record("setVertexBuffer", slot, buffer, offset, size ?? null)
  }
  setIndexBuffer(buffer: Resource, format: GPUIndexFormat, offset = 0, size?: number): void {
    this.record("setIndexBuffer", buffer, format, offset, size ?? null)
  }
  draw(vertices: number, instances = 1, firstVertex = 0, firstInstance = 0): void {
    this.record("draw", vertices, instances, firstVertex, firstInstance)
  }
  drawIndexed(
    indices: number,
    instances = 1,
    firstIndex = 0,
    baseVertex = 0,
    firstInstance = 0,
  ): void {
    this.record("drawIndexed", indices, instances, firstIndex, baseVertex, firstInstance)
  }
  drawIndirect(buffer: Resource, offset: number): void {
    this.record("drawIndirect", buffer, offset)
  }
  drawIndexedIndirect(buffer: Resource, offset: number): void {
    this.record("drawIndexedIndirect", buffer, offset)
  }
  setViewport(x: number, y: number, w: number, h: number, min: number, max: number): void {
    this.record("setViewport", x, y, w, h, min, max)
  }
  setScissorRect(x: number, y: number, w: number, h: number): void {
    this.record("setScissorRect", x, y, w, h)
  }
  setStencilReference(value: number): void {
    this.record("setStencilReference", value)
  }
  setBlendConstant(value: GPUColor): void {
    this.record("setBlendConstant", value)
  }
  beginOcclusionQuery(index: number): void {
    this.record("beginOcclusionQuery", index)
  }
  endOcclusionQuery(): void {
    this.record("endOcclusionQuery")
  }
  executeBundles(): never {
    throw new Error("Render bundles are not supported by the native wgpu binding")
  }
}
class ComputePass extends Pass {
  dispatchWorkgroups(x: number, y = 1, z = 1): void {
    this.record("dispatchWorkgroups", x, y, z)
  }
  dispatchWorkgroupsIndirect(buffer: Resource, offset: number): void {
    this.record("dispatchWorkgroupsIndirect", buffer, offset)
  }
}

function gpuError(error: unknown): GPUError {
  const message = error instanceof Error ? error.message : String(error)
  if (message.startsWith("out-of-memory:")) return new GPUOutOfMemoryError(message)
  if (message.startsWith("internal:")) return new GPUInternalError(message)
  return new GPUValidationError(message)
}

class Device extends EventTarget {
  readonly [nativeDevice]: NativeWgpuDevice
  readonly features: ReadonlySet<string>
  readonly limits: GPUSupportedLimits
  readonly lost: Promise<GPUDeviceLostInfo>
  readonly queue: GPUQueue
  readonly adapterInfo: GPUAdapterInfo
  label: string
  onuncapturederror: ((event: GPUUncapturedErrorEvent) => void) | null = null
  #scopes: { filter: GPUErrorFilter; error: GPUError | null }[] = []
  #timer: ReturnType<typeof setInterval>
  #resolveLost!: (info: GPUDeviceLostInfo) => void
  constructor(native: NativeWgpuDevice, info: GPUAdapterInfo, label = "") {
    super()
    this[nativeDevice] = native
    this.features = new Set(native.features)
    this.limits = native.limits as GPUSupportedLimits
    this.adapterInfo = info
    this.label = label
    this.lost = new Promise((resolve) => {
      this.#resolveLost = resolve
    })
    const weak = new WeakRef(this)
    const timer = setInterval(() => {
      const device = weak.deref()
      if (!device) {
        clearInterval(timer)
        return
      }
      for (const message of native.takeErrors()) device.report(gpuError(message))
      const loss = native.loss
      if (loss) {
        clearInterval(timer)
        device.#resolveLost(loss as GPUDeviceLostInfo)
      }
    }, 10)
    timer.unref()
    this.#timer = timer
    this.queue = {
      label: "",
      submit: (buffers: Iterable<GPUCommandBuffer>) =>
        this.call(() => native.submit(serialize(Array.from(buffers), this) as number[])),
      writeBuffer: (
        buffer: GPUBuffer,
        offset: number,
        data: AllowSharedBufferSource,
        dataOffset = 0,
        size?: number,
      ) =>
        this.call(() =>
          native.writeBuffer(
            serialize(buffer, this) as number,
            offset,
            bytes(data, dataOffset, size),
          ),
        ),
      writeTexture: (
        destination: GPUTexelCopyTextureInfo,
        data: AllowSharedBufferSource,
        layout: GPUTexelCopyBufferLayout,
        size: GPUExtent3D,
      ) =>
        this.call(() =>
          native.writeTexture(serialize(destination, this), bytes(data), layout, size),
        ),
      onSubmittedWorkDone: () => native.submittedWorkDone(),
      copyExternalImageToTexture: () => {
        throw new Error(
          "DOM/media uploads are not supported by native wgpu; use writeTexture with decoded pixels",
        )
      },
    } as unknown as GPUQueue
  }
  report(error: GPUError): void {
    const filter =
      error instanceof GPUOutOfMemoryError
        ? "out-of-memory"
        : error instanceof GPUInternalError
          ? "internal"
          : "validation"
    const scope = [...this.#scopes].reverse().find((s) => s.filter === filter)
    if (scope) {
      scope.error ??= error
      return
    }
    queueMicrotask(() => {
      const event = new GPUUncapturedErrorEvent("uncapturederror", { error })
      this.onuncapturederror?.(event)
      this.dispatchEvent(event)
    })
  }
  call<T>(fn: () => T): T | undefined {
    try {
      return fn()
    } catch (error) {
      this.report(gpuError(error))
      return undefined
    }
  }
  resource(kind: string, descriptor: Descriptor): Resource {
    const id = this.call(() => this[nativeDevice].create(kind, serialize(descriptor, this))) ?? 0
    const Class =
      kind === "buffer"
        ? BufferResource
        : kind === "texture"
          ? Texture
          : kind === "shader"
            ? Shader
            : kind === "querySet"
              ? QuerySet
              : kind.endsWith("Pipeline")
                ? Pipeline
                : Resource
    return new Class(this, id, descriptor)
  }
  createBuffer(d: GPUBufferDescriptor): Resource {
    return this.resource("buffer", d as unknown as Descriptor)
  }
  createTexture(d: GPUTextureDescriptor): Resource {
    return this.resource("texture", d as unknown as Descriptor)
  }
  createSampler(d: GPUSamplerDescriptor = {}): Resource {
    return this.resource("sampler", d as Descriptor)
  }
  createShaderModule(d: GPUShaderModuleDescriptor): Resource {
    return this.resource("shader", d as unknown as Descriptor)
  }
  createBindGroupLayout(d: GPUBindGroupLayoutDescriptor): Resource {
    return this.resource("bindGroupLayout", d as unknown as Descriptor)
  }
  createBindGroup(d: GPUBindGroupDescriptor): Resource {
    return this.resource("bindGroup", d as unknown as Descriptor)
  }
  createPipelineLayout(d: GPUPipelineLayoutDescriptor): Resource {
    return this.resource("pipelineLayout", d as unknown as Descriptor)
  }
  createRenderPipeline(d: GPURenderPipelineDescriptor): Resource {
    return this.resource("renderPipeline", d as unknown as Descriptor)
  }
  createComputePipeline(d: GPUComputePipelineDescriptor): Resource {
    return this.resource("computePipeline", d as unknown as Descriptor)
  }
  createQuerySet(d: GPUQuerySetDescriptor): Resource {
    return this.resource("querySet", d as unknown as Descriptor)
  }
  createCommandEncoder(d: GPUCommandEncoderDescriptor = {}): CommandEncoder {
    return new CommandEncoder(this, d)
  }
  async createRenderPipelineAsync(d: GPURenderPipelineDescriptor): Promise<Resource> {
    return this.pipelineAsync("renderPipeline", d)
  }
  async createComputePipelineAsync(d: GPUComputePipelineDescriptor): Promise<Resource> {
    return this.pipelineAsync("computePipeline", d)
  }
  private async pipelineAsync(kind: string, descriptor: object): Promise<Resource> {
    // Retain shaders and layouts until the worker has consumed the descriptor.
    const references = new Set<Resource>()
    try {
      const id = await this[nativeDevice].createPipelineAsync(
        kind,
        serialize(descriptor, this, references),
      )
      return new Pipeline(this, id as number, descriptor as Descriptor)
    } catch (error) {
      throw new GPUPipelineError(error instanceof Error ? error.message : String(error))
    } finally {
      references.clear()
    }
  }
  pushErrorScope(filter: GPUErrorFilter): void {
    if (!["validation", "out-of-memory", "internal"].includes(filter))
      throw new TypeError("Invalid GPU error filter")
    this.#scopes.push({ filter, error: null })
  }
  async popErrorScope(): Promise<GPUError | null> {
    const scope = this.#scopes.pop()
    if (!scope) throw new DOMException("No GPU error scope to pop", "OperationError")
    return scope.error
  }
  destroy(): void {
    clearInterval(this.#timer)
    this[nativeDevice].destroy()
    this.#resolveLost({ reason: "destroyed", message: "GPU device destroyed" } as GPUDeviceLostInfo)
  }
  createRenderBundleEncoder(): never {
    throw new Error("Render bundles are not supported by native wgpu")
  }
  importExternalTexture(): never {
    throw new Error("External textures are not supported by native wgpu")
  }
}

export function wrapDevice(native: NativeWgpuDevice, info: GPUAdapterInfo, label = ""): GPUDevice {
  return new Device(native, info, label) as unknown as GPUDevice
}
export function wrapAdapter(native: NativeWgpuAdapter): GPUAdapter {
  const info = native.info as GPUAdapterInfo
  let consumed = false
  return {
    features: new Set(native.features),
    limits: native.limits,
    info,
    requestDevice: async (descriptor: GPUDeviceDescriptor = {}) => {
      if (consumed)
        throw new DOMException("GPU adapter has already been consumed", "OperationError")
      consumed = true
      return wrapDevice(await native.requestDevice(descriptor), info, descriptor.label)
    },
  } as unknown as GPUAdapter
}

export const globals = {
  GPUBufferUsage: {
    MAP_READ: 1,
    MAP_WRITE: 2,
    COPY_SRC: 4,
    COPY_DST: 8,
    INDEX: 16,
    VERTEX: 32,
    UNIFORM: 64,
    STORAGE: 128,
    INDIRECT: 256,
    QUERY_RESOLVE: 512,
  },
  GPUTextureUsage: {
    COPY_SRC: 1,
    COPY_DST: 2,
    TEXTURE_BINDING: 4,
    STORAGE_BINDING: 8,
    RENDER_ATTACHMENT: 16,
  },
  GPUShaderStage: { VERTEX: 1, FRAGMENT: 2, COMPUTE: 4 },
  GPUMapMode: { READ: 1, WRITE: 2 },
  GPUColorWrite: { RED: 1, GREEN: 2, BLUE: 4, ALPHA: 8, ALL: 15 },
  GPUValidationError,
  GPUOutOfMemoryError,
  GPUInternalError,
  GPUPipelineError,
  GPUUncapturedErrorEvent,
  GPUDevice: Device,
  GPUBuffer: BufferResource,
  GPUTexture: Texture,
  GPUShaderModule: Shader,
}
