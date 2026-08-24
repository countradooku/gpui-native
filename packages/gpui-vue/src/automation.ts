import {
  InProcessBackend,
  liveRendererAsTest,
  serveAutomationStdio,
  type LiveAutomationRenderer,
} from "./automation-client.js"
import type { NativeNodeId, NativeRenderer } from "./native.js"
import type { StyleDesc } from "./types.js"

export interface AutomationNodeSnapshot {
  id: NativeNodeId
  kind: "element" | "text" | "comment"
  tag?: string
  text?: string
  parent: NativeNodeId | null
  children: NativeNodeId[]
  style: StyleDesc | null
  events: string[]
  customProps: Record<string, unknown>
  bounds?: {
    x: number
    y: number
    width: number
    height: number
  }
}

export interface AutomationTreeSnapshot {
  rootId: NativeNodeId | null
  revision: number
  nodes: ReadonlyMap<NativeNodeId, AutomationNodeSnapshot>
}

interface SerializedAutomationTree {
  rootId?: number | null
  revision?: number
  nodes?: Record<string, AutomationNodeSnapshot>
}

interface NativeAutomationNode {
  type: string
  id: number
  testId?: string
  text?: string
  bounds?: AutomationNodeSnapshot["bounds"]
  style?: StyleDesc
  events?: string[]
  customProps?: Record<string, unknown>
  children?: NativeAutomationNode[]
}

/** Reads a stable, serializable tree without depending on GPUI paint timing. */
export function snapshotRenderer(renderer: NativeRenderer): AutomationTreeSnapshot {
  const json = renderer.getAutomationTree?.() ?? renderer.snapshotJson?.()
  if (json === undefined) {
    throw new Error("This native renderer does not expose an automation snapshot")
  }
  const snapshot = JSON.parse(json) as SerializedAutomationTree
  const nodes = new Map<NativeNodeId, AutomationNodeSnapshot>()
  if (snapshot.nodes !== undefined) {
    for (const value of Object.values(snapshot.nodes)) {
      nodes.set(value.id, value)
    }
  } else if (isNativeAutomationNode(snapshot)) {
    flattenNativeAutomationNode(snapshot, null, nodes)
  }
  return {
    rootId: snapshot.rootId ?? (isNativeAutomationNode(snapshot) ? snapshot.id : null),
    revision: snapshot.revision ?? 0,
    nodes,
  }
}

function isNativeAutomationNode(value: unknown): value is NativeAutomationNode {
  if (value === null || typeof value !== "object") return false
  const node = value as Partial<NativeAutomationNode>
  return typeof node.type === "string" && typeof node.id === "number"
}

function flattenNativeAutomationNode(
  node: NativeAutomationNode,
  parent: NativeNodeId | null,
  nodes: Map<NativeNodeId, AutomationNodeSnapshot>,
): void {
  const children = node.children ?? []
  const customProps = { ...node.customProps }
  if (node.testId !== undefined) customProps.testId = node.testId
  nodes.set(node.id, {
    id: node.id,
    kind: node.type === "text" ? "text" : "element",
    ...(node.type === "text" ? {} : { tag: node.type }),
    ...(node.text === undefined ? {} : { text: node.text }),
    parent,
    children: children.map((child) => child.id),
    style: node.style ?? null,
    events: node.events ?? [],
    customProps,
    ...(node.bounds === undefined ? {} : { bounds: node.bounds }),
  })
  for (const child of children) flattenNativeAutomationNode(child, node.id, nodes)
}

export function automationText(
  snapshot: AutomationTreeSnapshot,
  id: NativeNodeId = snapshot.rootId ?? -1,
): string {
  const node = snapshot.nodes.get(id)
  if (node === undefined) return ""
  return [node.text ?? "", ...node.children.map((child) => automationText(snapshot, child))]
    .filter(Boolean)
    .join("")
}

export function findAutomationNodes(
  snapshot: AutomationTreeSnapshot,
  predicate: (node: AutomationNodeSnapshot) => boolean,
): AutomationNodeSnapshot[] {
  return [...snapshot.nodes.values()].filter(predicate)
}

export function findAutomationNodeByTestId(
  snapshot: AutomationTreeSnapshot,
  testId: string,
): AutomationNodeSnapshot | undefined {
  return findAutomationNodes(snapshot, (node) => node.customProps.testId === testId)[0]
}

export type AutomationTarget = NativeNodeId | string | AutomationNodeSnapshot

/** High-level automation facade over the native renderer's GPUI-backed hooks. */
export class GpuiAutomation {
  constructor(readonly renderer: NativeRenderer) {}

  snapshot(): AutomationTreeSnapshot {
    return snapshotRenderer(this.renderer)
  }

  findByTestId(testId: string): AutomationNodeSnapshot {
    const node = findAutomationNodeByTestId(this.snapshot(), testId)
    if (node === undefined) {
      throw new Error(`No native GPUI element has testId ${JSON.stringify(testId)}`)
    }
    return node
  }

  findByType(tag: string): AutomationNodeSnapshot[] {
    return findAutomationNodes(this.snapshot(), (node) => node.tag === tag)
  }

  bounds(target: AutomationTarget): NonNullable<AutomationNodeSnapshot["bounds"]> {
    const node = this.#node(target)
    const native = this.renderer.getElementBounds?.(node.id)
    if (native !== null && native !== undefined && native.length >= 4) {
      return {
        x: native[0] ?? 0,
        y: native[1] ?? 0,
        width: native[2] ?? 0,
        height: native[3] ?? 0,
      }
    }
    if (node.bounds !== undefined) return node.bounds
    throw new Error(`Native bounds are not available for GPUI element ${node.id}`)
  }

  click(target: AutomationTarget, button = 0): void {
    const bounds = this.bounds(target)
    this.renderer.simulateClick?.(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2, button)
  }

  mouseMove(x: number, y: number, pressedButton?: number): void {
    this.renderer.simulateMouseMove?.(x, y, pressedButton)
  }

  mouseDown(x: number, y: number, button = 0): void {
    this.renderer.simulateMouseDown?.(x, y, button)
  }

  mouseUp(x: number, y: number, button = 0): void {
    this.renderer.simulateMouseUp?.(x, y, button)
  }

  pauseClock(): number {
    return this.renderer.clockPause?.() ?? 0
  }

  setClock(nowMs: number): number {
    return this.renderer.clockSet?.(nowMs) ?? nowMs
  }

  fastForward(deltaMs: number): number {
    return this.renderer.clockFastForward?.(deltaMs) ?? deltaMs
  }

  resumeClock(): number {
    return this.renderer.clockResume?.() ?? 0
  }

  screenshot(path: string): void {
    if (this.renderer.captureScreenshot === undefined) {
      throw new Error("This GPUI renderer does not support screenshots")
    }
    this.renderer.captureScreenshot(path)
  }

  allText(): string[] {
    return this.renderer.getAllText?.() ?? []
  }

  paintedText(): string[] {
    return this.renderer.getPaintedText?.() ?? []
  }

  #node(target: AutomationTarget): AutomationNodeSnapshot {
    if (typeof target === "object") return target
    const snapshot = this.snapshot()
    const node =
      typeof target === "string"
        ? findAutomationNodeByTestId(snapshot, target)
        : snapshot.nodes.get(target)
    if (node === undefined) {
      throw new Error(`Unknown GPUI automation target ${JSON.stringify(target)}`)
    }
    return node
  }
}

/** Serve the same typed stdin/stdout automation protocol as the reference runtime. */
export function enableAutomation(renderer: LiveAutomationRenderer): void {
  serveAutomationStdio(new InProcessBackend(liveRendererAsTest(renderer)))
}

export {
  App,
  connectStdio,
  connectTest,
  handleAutomationRequest,
  InProcessBackend,
  launch,
  liveRendererAsTest,
  Locator,
  serveAutomationStdio,
  SseBackend,
} from "./automation-client.js"
export type {
  AutomationBackend,
  LiveAutomationRenderer,
  TestAutomationRenderer,
} from "./automation-client.js"
export {
  AutomationError,
  createSseDecoder,
  decodeSseChunk,
  encodeSse,
  methods,
  parseRequest,
  parseResponse,
  parseWireMessage,
  PROTOCOL_VERSION,
} from "./automation-protocol.js"
export type {
  AutomationErrorCode,
  AutomationRequest,
  AutomationResponse,
  AutomationServerEvent,
  ElementBounds,
  MethodName,
  ParamsOf,
  ResultOf,
  TreeNode,
  WireMessage,
} from "./automation-protocol.js"
