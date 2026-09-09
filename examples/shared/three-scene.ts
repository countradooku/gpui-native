import type { GPUCanvas } from "@gpui-native/runtime/webgpu"
import {
  BoxGeometry,
  DataTexture,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  RGBAFormat,
  Scene,
  WebGPURenderer,
} from "three/webgpu"

/** Decoded texture upload, depth testing, MSAA, resizing and explicit ownership. */
export async function startThreeScene(
  canvas: GPUCanvas,
  gpu: GPU,
  onError: (error: unknown) => void,
): Promise<() => void> {
  const device = await canvas.requestDevice(gpu)
  const renderer = new WebGPURenderer({
    canvas: canvas as unknown as HTMLCanvasElement,
    device,
    antialias: true,
  })
  const geometry = new BoxGeometry()
  const texture = new DataTexture(
    new Uint8Array([255, 210, 40, 255, 40, 210, 255, 255, 180, 65, 255, 255, 255, 100, 70, 255]),
    2,
    2,
    RGBAFormat,
  )
  texture.needsUpdate = true
  const material = new MeshBasicMaterial({ map: texture })
  const mesh = new Mesh(geometry, material)
  const scene = new Scene()
  scene.add(mesh)
  const camera = new PerspectiveCamera(45, canvas.width / canvas.height, 0.1, 10)
  camera.position.z = 3
  let stopped = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let width = 0,
    height = 0
  const stop = () => {
    if (stopped) return
    stopped = true
    clearTimeout(timer)
    geometry.dispose()
    material.dispose()
    texture.dispose()
    renderer.dispose()
    if (canvas.ownsDevice) device.destroy()
  }
  try {
    await renderer.init()
    if (canvas.destroyed) {
      stop()
      return stop
    }
    const frame = async () => {
      if (stopped || canvas.destroyed) {
        stop()
        return
      }
      try {
        if (width !== canvas.width || height !== canvas.height) {
          width = canvas.width
          height = canvas.height
          renderer.setSize(width, height, false)
          camera.aspect = width / height
          camera.updateProjectionMatrix()
        }
        mesh.rotation.x += 0.008
        mesh.rotation.y += 0.012
        renderer.render(scene, camera)
        await canvas.present()
        if (!stopped)
          timer = setTimeout(() => {
            void frame()
          }, 16)
      } catch (error) {
        if (!canvas.destroyed) onError(error)
        stop()
      }
    }
    void frame()
    return stop
  } catch (error) {
    stop()
    throw error
  }
}
