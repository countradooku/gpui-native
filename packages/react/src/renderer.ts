import { wrapWithBatching } from "@gpui-native/runtime/batching"
import { unregisterEventHandlers } from "@gpui-native/runtime/events"
import { MemoryNativeRenderer, type NativeRenderer } from "@gpui-native/runtime/native"
import { createPatchProp, GPUI_ELEMENT_TYPES } from "@gpui-native/runtime/nodeOps"
import {
  createGpuiRoot,
  type GpuiElement,
  type GpuiContainer,
  type GpuiNode,
} from "@gpui-native/runtime/nodes"
import { allocateRendererId, claimRenderer } from "@gpui-native/runtime/ownership"
import type { GpuiElementType } from "@gpui-native/runtime/types"
import { createContext, createElement, type ReactNode, type ReactPortal } from "react"
import Reconciler from "react-reconciler"
import { ConcurrentRoot, DefaultEventPriority } from "react-reconciler/constants.js"

import { GpuiRendererContext } from "./context.js"

type Props = Record<string, unknown>
type Instance = Omit<GpuiElement, "props"> & { props: Props; hidden?: boolean }
type Container = GpuiContainer

// Render-phase work is entirely local. Only resetAfterCommit touches the backend.
function detach(child: GpuiNode) {
  const parent = child.parent
  if (!parent) return
  if (child.previousSibling) child.previousSibling.nextSibling = child.nextSibling
  else parent.firstChild = child.nextSibling
  if (child.nextSibling) child.nextSibling.previousSibling = child.previousSibling
  else parent.lastChild = child.previousSibling
  child.parent = null
  child.previousSibling = child.nextSibling = null
}
function insert(parent: Container, child: GpuiNode, before: GpuiNode | null = null) {
  if (child === before) return
  detach(child)
  child.parent = parent
  child.previousSibling = before ? before.previousSibling : parent.lastChild
  child.nextSibling = before
  if (child.previousSibling) child.previousSibling.nextSibling = child
  else parent.firstChild = child
  if (before) before.previousSibling = child
  else parent.lastChild = child
}
export interface GpuiRendererOptions {
  onError?: (error: unknown) => void
  strictMode?: boolean
}
export function createGpuiRenderer(
  nativeRenderer: NativeRenderer = new MemoryNativeRenderer(),
  options: GpuiRendererOptions = {},
) {
  const release = claimRenderer(nativeRenderer)
  const renderer = wrapWithBatching(nativeRenderer)
  const root = createGpuiRoot(renderer, allocateRendererId(nativeRenderer))
  const patch = createPatchProp(renderer)
  type Snapshot = { node: Instance; props: Props; children: number[]; parent: number | null }
  let committed = new Map<number, Snapshot>()
  let rootCreated = false
  let destroyed = false
  let priority = DefaultEventPriority
  let failure: unknown
  const report = (error: unknown) => {
    failure = error
    options.onError?.(error)
  }
  const sync = () => {
    const next = new Map<number, Snapshot>()
    if (!rootCreated) {
      renderer.createElement(root.id, "div")
      renderer.setStyle(root.id, { width: "100%", height: "100%" })
      renderer.setRoot(root.id)
      rootCreated = true
    }
    // React separates interpolations into text fibers. Coalesce adjacent fibers
    // into one native text run so they share shaping, selection and line layout.
    type NativeChild = { node: Instance; text?: string }
    const groups = new Map<number, NativeChild[]>()
    const nativeChildren = (parent: Container): NativeChild[] => {
      const cached = groups.get(parent.id)
      if (cached) return cached
      const children: NativeChild[] = []
      for (const child of parent.children as Instance[]) {
        const text = child.props.__text
        const previous = children.at(-1)
        if (
          typeof text === "string" &&
          previous?.text !== undefined &&
          previous.node.hidden === child.hidden
        ) {
          previous.text += text
        } else children.push({ node: child, ...(typeof text === "string" ? { text } : {}) })
      }
      groups.set(parent.id, children)
      return children
    }
    const visit = ({ node, text }: NativeChild) => {
      const old = committed.get(node.id)
      const props: Props = node.hidden
        ? { ...node.props, style: { ...(node.props.style as object), display: "none" } }
        : { ...node.props }
      if (text !== undefined) props.__text = text
      if (!old) renderer.createElement(node.id, node.tag)
      // patchProp maintains a local prop cache; don't let it alter React's committed props.
      const target = { ...node, props: { ...node.props } }
      if (node.tag === "text" && props.__text !== undefined) {
        if (old?.props.__text !== props.__text) renderer.setText(node.id, String(props.__text))
      }
      for (const key of new Set([...Object.keys(old?.props ?? {}), ...Object.keys(props)])) {
        if (key === "children" || key === "ref" || key === "__text") continue
        if (!Object.is(old?.props[key], props[key])) patch(target, key, old?.props[key], props[key])
      }
      next.set(node.id, {
        node,
        props,
        children: nativeChildren(node).map(({ node }) => node.id),
        parent: node.parent?.id ?? null,
      })
      nativeChildren(node).forEach(visit)
    }
    nativeChildren(root).forEach(visit)
    // Detach removed branches before reordering retained siblings.
    for (const [id, old] of committed) {
      if (next.has(id)) continue
      unregisterEventHandlers(id, renderer)
      if (old.parent === root.id || (old.parent !== null && next.has(old.parent)))
        renderer.destroyElement(id)
    }
    const order = (parent: Container) => {
      const oldChildren =
        parent === root ? previousRootChildren : (committed.get(parent.id)?.children ?? [])
      const children = nativeChildren(parent).map(({ node }) => node)
      if (
        oldChildren.length !== children.length ||
        children.some((child, index) => oldChildren[index] !== child.id)
      ) {
        children.forEach((child) => renderer.appendChild(parent.id, child.id))
      }
      children.forEach((child) => order(child as Container))
    }
    order(root)
    renderer.flushMutations()
    committed = next
    previousRootChildren = nativeChildren(root).map(({ node }) => node.id)
  }
  let previousRootChildren: number[] = []
  const instance = (tag: string, props: Props): Instance => {
    if (!(GPUI_ELEMENT_TYPES as readonly string[]).includes(tag))
      throw new Error(`Unsupported GPUI element tag: ${tag}`)
    const base = createGpuiRoot(renderer, allocateRendererId(nativeRenderer))
    return {
      ...base,
      kind: "element",
      tag: tag as GpuiElementType,
      props,
      get children() {
        const result: GpuiNode[] = []
        let node = this.firstChild
        while (node) {
          result.push(node)
          node = node.nextSibling
        }
        return result
      },
    }
  }
  const noop = () => {}
  const reconciler = Reconciler<
    string,
    Props,
    Container,
    Instance,
    Instance,
    never,
    never,
    Instance,
    Instance,
    object,
    never,
    ReturnType<typeof setTimeout>,
    -1,
    null
  >({
    supportsMutation: true,
    supportsPersistence: false,
    supportsHydration: false,
    isPrimaryRenderer: false,
    warnsIfNotActing: true,
    getRootHostContext: () => hostContext,
    getChildHostContext: (context) => context,
    getPublicInstance: (node) => node,
    prepareForCommit: () => null,
    resetAfterCommit: sync,
    createInstance: (tag, props) => instance(tag, props),
    createTextInstance: (text) => instance("text", { __text: text }),
    appendInitialChild: insert,
    appendChild: insert,
    appendChildToContainer: insert,
    insertBefore: insert,
    insertInContainerBefore: insert,
    removeChild: (_parent, child) => detach(child),
    removeChildFromContainer: (_parent, child) => detach(child),
    finalizeInitialChildren: () => false,
    shouldSetTextContent: () => false,
    commitUpdate: (node, _tag, _old, props) => {
      node.props = props
    },
    commitTextUpdate: (node, _old, text) => {
      node.props = { __text: text }
    },
    resetTextContent: noop,
    commitMount: noop,
    hideInstance: (node) => {
      node.hidden = true
    },
    unhideInstance: (node) => {
      node.hidden = false
    },
    hideTextInstance: (node) => {
      node.hidden = true
    },
    unhideTextInstance: (node) => {
      node.hidden = false
    },
    clearContainer: (container) => {
      container.children.forEach(detach)
    },
    scheduleTimeout: setTimeout,
    cancelTimeout: clearTimeout,
    noTimeout: -1,
    supportsMicrotasks: true,
    scheduleMicrotask: queueMicrotask,
    preparePortalMount: noop,
    detachDeletedInstance: noop,
    getInstanceFromNode: () => null,
    beforeActiveInstanceBlur: noop,
    afterActiveInstanceBlur: noop,
    prepareScopeUpdate: noop,
    getInstanceFromScope: () => null,
    NotPendingTransition: null,
    HostTransitionContext: createContext(null) as unknown as Reconciler.ReactContext<null>,
    setCurrentUpdatePriority: (value) => {
      priority = value
    },
    getCurrentUpdatePriority: () => priority,
    resolveUpdatePriority: () => priority || DefaultEventPriority,
    resetFormInstance: noop,
    requestPostPaintCallback: (callback) => {
      setTimeout(() => callback(performance.now()), 0)
    },
    shouldAttemptEagerTransition: () => false,
    trackSchedulerEvent: noop,
    resolveEventType: () => null,
    resolveEventTimeStamp: () => performance.now(),
    ...commitSuspension,
    maySuspendCommit: () => false,
    preloadInstance: () => true,
    startSuspendingCommit: noop,
    suspendInstance: noop,
    waitForCommitToBeReady: () => null,
  })
  const fiber = reconciler.createContainer(
    root,
    ConcurrentRoot,
    null,
    options.strictMode ?? false,
    null,
    "gpui-",
    report,
    noop,
    report,
    noop,
  )
  const render = (node: ReactNode) => {
    if (destroyed) throw new Error("Cannot render into a destroyed GPUI root")
    failure = undefined
    reconciler.flushSyncFromReconciler(() =>
      reconciler.updateContainer(
        createElement(GpuiRendererContext.Provider, { value: renderer }, node),
        fiber,
        null,
        null,
      ),
    )
    reconciler.flushPassiveEffects()
    if (failure !== undefined) throw failure
  }
  return {
    nativeRenderer,
    renderer,
    root,
    render,
    mount: render,
    createPortal(children: ReactNode, container: GpuiElement, key?: string): ReactPortal {
      if (container.renderer !== renderer)
        throw new Error("Portals must target an element in the same GPUI root")
      return reconciler.createPortal(children, container, null, key) as unknown as ReactPortal
    },
    flushMutations: () => renderer.flushMutations(),
    flush: () => {
      reconciler.flushSyncWork()
      reconciler.flushPassiveEffects()
      renderer.flushMutations()
    },
    destroy() {
      if (destroyed) return
      render(null)
      renderer.destroyElement(root.id)
      renderer.flushMutations()
      destroyed = true
      release()
    },
  }
}
const hostContext = Object.freeze({})
export type GpuiRendererHost = ReturnType<typeof createGpuiRenderer>
export type { GpuiElement, GpuiNode, GpuiPublicInstance } from "@gpui-native/runtime/nodes"

const commitSuspension = {
  maySuspendCommitOnUpdate: () => false,
  maySuspendCommitInSyncRender: () => false,
  getSuspendedCommitReason: () => null,
}
