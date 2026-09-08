import { describe, expect, it, vi } from "vitest"

import { MemoryNativeRenderer } from "../src/native.js"
import { createGPUCanvas } from "../src/webgpu.js"

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
function fixture(maxFramesInFlight = 2) {
  const publish = vi.fn<(...args: unknown[]) => void>()
  const destroy = vi.fn<(id: number) => void>()
  const renderer = Object.assign(new MemoryNativeRenderer(), {
    createCanvasSource: () => 7,
    presentCanvasFrame: publish,
    destroyCanvasSource: destroy,
  })
  const lost = deferred<Pick<GPUDeviceLostInfo, "reason" | "message">>()
  const buffers: {
    ready: ReturnType<typeof deferred<void>>
    destroy: ReturnType<typeof vi.fn>
    data: ArrayBuffer
    mapState: string
  }[] = []
  const textures: { destroy: ReturnType<typeof vi.fn> }[] = []
  const device = {
    limits: { maxTextureDimension2D: 8192 },
    lost: lost.promise,
    createBuffer: ({ size }: { size: number }) => {
      const buffer = {
        ready: deferred<void>(),
        data: new ArrayBuffer(size),
        mapState: "unmapped",
        destroy: vi.fn<() => void>(() => {
          buffer.mapState = "unmapped"
        }),
        mapAsync: () => {
          buffer.ready = deferred<void>()
          buffer.mapState = "pending"
          return buffer.ready.promise.then(() => {
            buffer.mapState = "mapped"
          })
        },
        getMappedRange: () => buffer.data,
        unmap: () => {
          buffer.mapState = "unmapped"
        },
      }
      buffers.push(buffer)
      return buffer
    },
    createTexture: vi.fn<() => { destroy: ReturnType<typeof vi.fn> }>(() => {
      const texture = { destroy: vi.fn<() => void>() }
      textures.push(texture)
      return texture
    }),
    createCommandEncoder: () => ({ copyTextureToBuffer: vi.fn<() => void>(), finish: () => ({}) }),
    queue: { submit: vi.fn<() => void>() },
  } as unknown as GPUDevice
  const canvas = createGPUCanvas({ renderer, width: 65, height: 2, maxFramesInFlight })
  canvas.configure({ device, format: "rgba8unorm" })
  return { canvas, device, lost, buffers, textures, publish, destroy }
}

describe("GPU canvas lifecycle and backpressure", () => {
  it("bounds readbacks, drops saturation, and ignores out-of-order completions", async () => {
    const f = fixture()
    const a = f.canvas.present(),
      b = f.canvas.present()
    expect(await f.canvas.present()).toBe(false)
    expect(f.buffers).toHaveLength(2)
    f.buffers[1]!.ready.resolve()
    expect(await b).toBe(true)
    f.buffers[0]!.ready.resolve()
    expect(await a).toBe(false)
    expect(f.publish).toHaveBeenCalledTimes(1)
    expect(f.publish.mock.calls[0]!.slice(0, 4)).toEqual([7, 65, 2, 512])
    expect(f.canvas.stats).toMatchObject({ submitted: 2, presented: 1, dropped: 2, inFlight: 0 })
    f.canvas.destroy()
  })

  it("reuses buffers and textures between frames and separates stats snapshots", async () => {
    const f = fixture(1)
    const snapshot = f.canvas.stats
    for (let i = 0; i < 3; i++) {
      const frame = f.canvas.present()
      f.buffers[0]!.ready.resolve()
      expect(await frame).toBe(true)
    }
    expect(f.buffers).toHaveLength(1)
    expect(f.textures).toHaveLength(1)
    expect(snapshot.presented).toBe(0)
    expect(f.canvas.stats.presented).toBe(3)
    f.canvas.destroy()
  })

  it("discards pending frames across resize and disposal, releasing resources once", async () => {
    const f = fixture(1)
    const frame = f.canvas.present()
    f.canvas.resize(128, 4)
    expect(f.buffers[0]!.destroy).toHaveBeenCalledTimes(1)
    expect(await f.canvas.present()).toBe(false)
    f.buffers[0]!.ready.resolve()
    expect(await frame).toBe(false)
    const next = f.canvas.present()
    f.canvas.destroy()
    f.canvas.destroy()
    f.buffers[1]!.ready.reject(new Error("destroyed while mapping"))
    expect(await next).toBe(false)
    expect(f.publish).not.toHaveBeenCalled()
    expect(f.destroy).toHaveBeenCalledOnce()
    expect(f.canvas.stats.inFlight).toBe(0)
    expect(() => f.canvas.getContext("webgpu").getCurrentTexture()).toThrow("destroyed")
  })

  it("reports real readback failures and recovers with a new buffer", async () => {
    const f = fixture(1)
    const failed = f.canvas.present()
    f.buffers[0]!.ready.reject(new Error("mapping failed"))
    await expect(failed).rejects.toThrow("mapping failed")
    const next = f.canvas.present()
    f.buffers[1]!.ready.resolve()
    expect(await next).toBe(true)
    expect(f.canvas.stats).toMatchObject({ failed: 1, inFlight: 0 })
    f.canvas.destroy()
  })

  it("reports device loss after resizing and permits configuration with a replacement device", async () => {
    const f = fixture()
    const listener = vi.fn<() => void>()
    f.canvas.addEventListener("contextlost", listener)
    f.canvas.resize(32, 32)
    f.lost.resolve({ reason: "destroyed", message: "lost" })
    await Promise.resolve()
    expect(listener).toHaveBeenCalledOnce()
    expect(f.canvas.getContext("webgpu").getConfiguration()).toBeNull()
    const replacement = fixture()
    f.canvas.configure({
      device: replacement.device,
      format: "bgra8unorm",
      alphaMode: "premultiplied",
    })
    expect(f.canvas.getContext("webgpu").getConfiguration()?.format).toBe("bgra8unorm")
    f.canvas.destroy()
    replacement.canvas.destroy()
  })

  it("rejects invalid sizes and unsupported color formats before replacing live configuration", () => {
    const f = fixture()
    for (const size of [0, -1, NaN, Infinity, 8193, 1.5])
      expect(() => f.canvas.resize(size, 1)).toThrow("dimensions")
    expect(() => f.canvas.resize(8192, 8192)).toThrow("64 MiB")
    expect(() => f.canvas.configure({ device: f.device, format: "rgba16float" })).toThrow(
      "supports",
    )
    expect(() =>
      f.canvas.configure({ device: f.device, format: "rgba8unorm", colorSpace: "display-p3" }),
    ).toThrow("sRGB")
    expect(f.canvas.getContext("2d")).toBeNull()
    expect(f.canvas.getContext("webgpu").getConfiguration()?.format).toBe("rgba8unorm")
    f.canvas.destroy()
  })
})
