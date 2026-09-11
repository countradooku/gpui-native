import { wrapWithBatching, type BatchingRenderer } from "@gpui-native/runtime/batching"
import type { NativeRenderer } from "@gpui-native/runtime/native"
import { createNodeOps, createPatchProp, equalJsonValue } from "@gpui-native/runtime/nodeOps"
import { createGpuiRoot, type GpuiContainer, type GpuiNode } from "@gpui-native/runtime/nodes"
import { allocateRendererId } from "@gpui-native/runtime/ownership"

/** Svelte's structural node interface. These objects never lay out or paint. */
export class HostNode extends EventTarget {
  readonly nodeType: number
  readonly nodeName: string
  parentNode: HostNode | null = null
  private first: HostNode | null = null
  private last: HostNode | null = null
  private previous: HostNode | null = null
  private next: HostNode | null = null
  private content = ""
  controller: HostController | null = null
  native: GpuiNode | null = null
  props: Record<string, unknown> = {}
  constructor(type: number, name: string, content = "") {
    super()
    this.nodeType = type
    this.nodeName = name
    this.content = content
  }
  get firstChild() {
    return this.first
  }
  get lastChild() {
    return this.last
  }
  get previousSibling() {
    return this.previous
  }
  get nextSibling() {
    return this.next
  }
  get ownerDocument() {
    return nativeDocument
  }
  get childNodes(): HostNode[] {
    const nodes: HostNode[] = []
    for (let node = this.first; node; node = node.next) nodes.push(node)
    return nodes
  }
  get isConnected(): boolean {
    return this.rootController() !== null
  }
  get data() {
    return this.content
  }
  set data(value: string) {
    if (this.content === value) return
    this.content = value
    this.controller?.markValue(this)
  }
  get nodeValue() {
    return this.nodeType === 3 || this.nodeType === 8 ? this.data : null
  }
  set nodeValue(value: string | null) {
    this.data = value ?? ""
  }
  get textContent(): string {
    return this.nodeType === 3 || this.nodeType === 8
      ? this.data
      : this.childNodes.map((n) => (n.nodeType === 8 ? "" : n.textContent)).join("")
  }
  set textContent(value: string) {
    if (this.nodeType === 3 || this.nodeType === 8) {
      this.data = value
      return
    }
    this.replaceChildren(...(value ? [new HostText(value)] : []))
  }
  rootController(): HostController | null {
    const node = this.getRootNode()
    return node.controller?.target === node ? node.controller : null
  }
  getRootNode(): HostNode {
    let node = this.parentNode ?? this
    while (node.parentNode) node = node.parentNode
    return node
  }
  contains(other: HostNode | null): boolean {
    for (let node = other; node; node = node.parentNode) if (node === this) return true
    return false
  }
  private detach() {
    const parent = this.parentNode
    if (!parent) return
    if (this.previous) this.previous.next = this.next
    else parent.first = this.next
    if (this.next) this.next.previous = this.previous
    else parent.last = this.previous
    this.parentNode = this.previous = this.next = null
    parent.controller?.markStructure(parent)
  }
  insertBefore<T extends HostNode>(node: T, anchor: HostNode | null): T {
    if (node === anchor) return node
    if (anchor && anchor.parentNode !== this) throw new Error("Invalid Svelte host anchor")
    if (node.contains(this)) throw new Error("Cannot insert an ancestor into its descendant")
    if (node.nodeType === 11) {
      for (const child of node.childNodes) this.insertBefore(child, anchor)
      return node
    }
    const owner = this.rootController()
    if (owner && node.controller && node.controller !== owner)
      throw new Error("Cannot move Svelte nodes between GPUI roots")
    node.detach()
    node.parentNode = this
    node.previous = anchor ? anchor.previous : this.last
    node.next = anchor
    if (node.previous) node.previous.next = node
    else this.first = node
    if (anchor) anchor.previous = node
    else this.last = node
    this.controller?.markStructure(this)
    return node
  }
  appendChild<T extends HostNode>(node: T): T {
    return this.insertBefore(node, null)
  }
  removeChild<T extends HostNode>(node: T): T {
    if (node.parentNode !== this) throw new Error("Not a child")
    node.remove()
    return node
  }
  append(...nodes: (HostNode | string)[]) {
    for (const node of nodes) this.appendChild(typeof node === "string" ? new HostText(node) : node)
  }
  before(...nodes: (HostNode | string)[]) {
    for (const node of nodes)
      this.parentNode?.insertBefore(typeof node === "string" ? new HostText(node) : node, this)
  }
  after(...nodes: (HostNode | string)[]) {
    const anchor = this.next
    for (const node of nodes)
      this.parentNode?.insertBefore(typeof node === "string" ? new HostText(node) : node, anchor)
  }
  remove() {
    this.detach()
  }
  replaceChildren(...nodes: (HostNode | string)[]) {
    while (this.first) this.first.remove()
    this.append(...nodes)
  }
  replaceWith(...nodes: (HostNode | string)[]) {
    this.before(...nodes)
    this.remove()
  }
  cloneNode(deep = false): HostNode {
    const node =
      this.nodeType === 3
        ? new HostText(this.data)
        : this.nodeType === 8
          ? new HostComment(this.data)
          : this.nodeType === 11
            ? new HostNode(11, "#document-fragment")
            : new HostElement(this.nodeName.toLowerCase())
    node.props = { ...this.props }
    if (deep) for (const child of this.childNodes) node.appendChild(child.cloneNode(true))
    return node
  }
  setProps(props: Record<string, unknown>) {
    this.props = props
    this.controller?.markValue(this)
  }
}
export class HostText extends HostNode {
  constructor(value = "") {
    super(3, "#text", value)
  }
}
export class HostComment extends HostNode {
  constructor(value = "") {
    super(8, "#comment", value)
  }
}
export class HostElement extends HostNode {
  constructor(readonly tagName: string) {
    super(1, tagName.toUpperCase())
  }
  get attributes() {
    return Object.entries(this.props).map(([name, value]) => ({ name, value }))
  }
  setAttribute(name: string, value: unknown) {
    this.setProps({ ...this.props, [name]: value })
  }
  getAttribute(name: string) {
    return this.props[name] ?? null
  }
  removeAttribute(name: string) {
    const props = { ...this.props }
    delete props[name]
    this.setProps(props)
  }
  hasAttribute(name: string) {
    return Object.hasOwn(this.props, name)
  }
}
class HostDocument extends HostNode {
  body = new HostElement("div")
  head = new HostElement("div")
  activeElement: HostNode = this.body
  contentType = "text/html"
  constructor() {
    super(9, "#document")
  }
  createTextNode(value: string) {
    return new HostText(value)
  }
  createComment(value: string) {
    return new HostComment(value)
  }
  createDocumentFragment() {
    return new HostNode(11, "#document-fragment")
  }
  createElement(name: string) {
    return new HostElement(name)
  }
  createElementNS(_namespace: string, name: string) {
    return new HostElement(name)
  }
  importNode(node: HostNode, deep: boolean) {
    return node.cloneNode(deep)
  }
}
export const nativeDocument = new HostDocument()
export const nativeWindow = Object.assign(new EventTarget(), { document: nativeDocument })
export const nativeNavigator = { userAgent: "GPUI Svelte" }

/** Tracks only changed parents and values; never snapshots the complete tree per update. */
export class HostController {
  readonly renderer: BatchingRenderer
  readonly root: GpuiContainer
  readonly target = new HostElement("div")
  private readonly ops: ReturnType<typeof createNodeOps>
  private readonly patch: ReturnType<typeof createPatchProp>
  private readonly structures = new Set<HostNode>()
  private readonly values = new Set<HostNode>()
  private readonly nodes = new Set<HostNode>()
  private readonly committed = new Map<HostNode, HostNode[]>()
  private scheduled = false
  disposed = false
  onError: (error: unknown) => void = (error) => {
    throw error
  }
  constructor(native: NativeRenderer) {
    this.renderer = wrapWithBatching(native)
    const allocate = () => allocateRendererId(native)
    this.ops = createNodeOps(this.renderer, allocate)
    this.patch = createPatchProp(this.renderer)
    this.root = createGpuiRoot(this.renderer, allocate())
    this.renderer.createElement(this.root.id, "div")
    this.renderer.setStyle(this.root.id, { width: "100%", height: "100%" })
    this.renderer.setRoot(this.root.id)
    this.target.native = this.root
    this.target.controller = this
  }
  private schedule() {
    if (this.scheduled || this.disposed) return
    this.scheduled = true
    queueMicrotask(() => {
      if (this.disposed) return
      try {
        this.flush()
      } catch (error) {
        this.onError(error)
      }
    })
  }
  markStructure(node: HostNode) {
    this.structures.add(node)
    this.schedule()
  }
  markValue(node: HostNode) {
    this.values.add(node)
    this.schedule()
  }
  private ensure(node: HostNode): GpuiNode | null {
    if (node.nodeType === 8) return null
    if (node.native) return node.native
    node.controller = this
    node.native =
      node.nodeType === 3
        ? this.ops.createText(node.data)
        : this.ops.createElement(node.nodeName.toLowerCase())
    this.nodes.add(node)
    this.update(node)
    if (node.nodeType === 1) this.structures.add(node)
    return node.native
  }
  private update(node: HostNode) {
    const target = node.native
    if (!target) return
    if (target.kind === "text") {
      this.ops.setText(target, node.data)
      return
    }
    if (target.kind !== "element") return
    const next = node.props
    for (const key of Object.keys(target.props))
      if (!Object.hasOwn(next, key)) this.patch(target, key, target.props[key], undefined)
    for (const key of Object.keys(next))
      if (!equalJsonValue(target.props[key], next[key]))
        this.patch(target, key, target.props[key], next[key])
  }
  flush() {
    this.scheduled = false
    if (this.disposed) return
    const removed = new Set<HostNode>()
    for (const parent of this.structures) {
      this.structures.delete(parent)
      if (parent !== this.target && parent.rootController() !== this) continue
      const target = parent.native as GpuiContainer | null
      if (!target) continue
      const children = parent.childNodes.filter((node) => node.nodeType !== 8)
      const next = new Set(children)
      for (const old of this.committed.get(parent) ?? []) if (!next.has(old)) removed.add(old)
      let anchor: GpuiNode | null = null
      for (let i = children.length - 1; i >= 0; i--) {
        const child = this.ensure(children[i]!)
        if (!child) continue
        if (child.parent !== target || child.nextSibling !== anchor)
          this.ops.insert(child, target, anchor)
        anchor = child
      }
      this.committed.set(parent, children)
    }
    for (const node of removed)
      if (node.rootController() !== this && node.native) {
        this.ops.remove(node.native)
        const clean = (node: HostNode) => {
          this.nodes.delete(node)
          this.committed.delete(node)
          this.values.delete(node)
          node.native = null
          node.controller = null
          for (const child of node.childNodes) clean(child)
        }
        clean(node)
      }
    for (const node of this.values) if (node.rootController() === this) this.update(node)
    this.values.clear()
    this.renderer.flushMutations()
  }
  dispose() {
    if (this.disposed) return
    this.disposed = true
    this.ops.setElementText(this.root, "")
    this.renderer.destroyElement(this.root.id)
    this.renderer.flushMutations()
    for (const node of this.nodes) {
      node.controller = null
      node.native = null
    }
    this.nodes.clear()
    this.structures.clear()
    this.values.clear()
    this.committed.clear()
    this.target.controller = null
  }
}
