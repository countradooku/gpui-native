import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { mkdir } from "node:fs/promises"
import path from "node:path"

import {
  AutomationError,
  createSseDecoder,
  encodeSse,
  methods,
  parseResponse,
  parseWireMessage,
  PROTOCOL_VERSION,
  type AutomationRequest,
  type AutomationResponse,
  type AutomationServerEvent,
  type ElementBounds,
  type MethodName,
  type ParamsOf,
  type ResultOf,
  type TreeNode,
} from "./automation-protocol.js"

export interface AutomationBackend {
  call<M extends MethodName>(method: M, params: ParamsOf<M>): Promise<ResultOf<M>>
  close(): Promise<void>
}

export interface TestAutomationRenderer {
  nativeSimulateClick(x: number, y: number): void
  nativeSimulateMouseDown(x: number, y: number, button?: number): void
  nativeSimulateMouseUp(x: number, y: number, button?: number): void
  nativeSimulateMouseMove(x: number, y: number, pressedButton?: number): void
  nativeSimulateScrollWheel(x: number, y: number, deltaX: number, deltaY: number): void
  simulateKeystrokes(keystrokes: string): void
  nativeSimulateKeystrokes(elementId: number, keystrokes: string): void
  nativeSimulateKeyDown(elementId: number, key: string, isHeld?: boolean): void
  nativeSimulateKeyUp(elementId: number, key: string): void
  scrollTo(elementId: number, x: number, y: number): void
  getScrollOffset(elementId: number): [number, number] | null
  getAllText(): string[]
  getPaintedText(): string[]
  getSelectedText(): string | null
  clearSelection(): void
  captureScreenshot(path: string): void
  getAutomationTree(): string
  getElementBounds(elementId: number): number[] | null
  clockPause(): number
  clockSet(nowMs: number): number
  clockFastForward(deltaMs: number): number
  clockResume(): number
  focusElement?(elementId: number): void
  blur?(): void
}

function ok(): { ok: true } {
  return { ok: true }
}

/** Executes the wire protocol directly against a native or GPU-backed test renderer. */
export class InProcessBackend implements AutomationBackend {
  constructor(readonly renderer: TestAutomationRenderer) {}

  async call<M extends MethodName>(method: M, params: ParamsOf<M>): Promise<ResultOf<M>> {
    const parsed = methods[method].params.parse(params)
    let result: unknown

    switch (method) {
      case "initialize":
        result = {
          protocolVersion: PROTOCOL_VERSION,
          pid: process.pid,
          capabilities: ["input", "screenshot", "clock", "tree"],
          window: { width: 800, height: 600 },
        }
        break
      case "cancel":
        result = ok()
        break
      case "click": {
        const input = parsed as ParamsOf<"click">
        this.renderer.nativeSimulateClick(input.x, input.y)
        result = ok()
        break
      }
      case "mouseDown": {
        const input = parsed as ParamsOf<"mouseDown">
        this.renderer.nativeSimulateMouseDown(input.x, input.y, input.button)
        result = ok()
        break
      }
      case "mouseUp": {
        const input = parsed as ParamsOf<"mouseUp">
        this.renderer.nativeSimulateMouseUp(input.x, input.y, input.button)
        result = ok()
        break
      }
      case "mouseMove": {
        const input = parsed as ParamsOf<"mouseMove">
        this.renderer.nativeSimulateMouseMove(input.x, input.y, input.pressedButton)
        result = ok()
        break
      }
      case "scrollWheel": {
        const input = parsed as ParamsOf<"scrollWheel">
        this.renderer.nativeSimulateScrollWheel(input.x, input.y, input.deltaX, input.deltaY)
        result = ok()
        break
      }
      case "keystrokes": {
        const input = parsed as ParamsOf<"keystrokes">
        if (input.elementId === undefined) this.renderer.simulateKeystrokes(input.keys)
        else this.renderer.nativeSimulateKeystrokes(input.elementId, input.keys)
        result = ok()
        break
      }
      case "keyDown": {
        const input = parsed as ParamsOf<"keyDown">
        this.renderer.nativeSimulateKeyDown(input.elementId ?? 0, input.key, input.isHeld)
        result = ok()
        break
      }
      case "keyUp": {
        const input = parsed as ParamsOf<"keyUp">
        this.renderer.nativeSimulateKeyUp(input.elementId ?? 0, input.key)
        result = ok()
        break
      }
      case "focus": {
        const input = parsed as ParamsOf<"focus">
        this.renderer.focusElement?.(input.elementId)
        result = ok()
        break
      }
      case "blur":
        this.renderer.blur?.()
        result = ok()
        break
      case "scrollTo": {
        const input = parsed as ParamsOf<"scrollTo">
        this.renderer.scrollTo(input.elementId, input.x, input.y)
        result = ok()
        break
      }
      case "getScrollOffset": {
        const input = parsed as ParamsOf<"getScrollOffset">
        result = { offset: this.renderer.getScrollOffset(input.elementId) }
        break
      }
      case "getTree": {
        const tree = JSON.parse(this.renderer.getAutomationTree()) as TreeNode | null
        result = { tree }
        break
      }
      case "getPaintedText":
        result = { text: this.renderer.getPaintedText() }
        break
      case "getAllText":
        result = { text: this.renderer.getAllText() }
        break
      case "getBounds": {
        const input = parsed as ParamsOf<"getBounds">
        const bounds = this.renderer.getElementBounds(input.elementId)
        result = {
          bounds:
            bounds === null
              ? null
              : {
                  x: bounds[0] ?? 0,
                  y: bounds[1] ?? 0,
                  width: bounds[2] ?? 0,
                  height: bounds[3] ?? 0,
                },
        }
        break
      }
      case "getSelectedText":
        result = { text: this.renderer.getSelectedText() }
        break
      case "clearSelection":
        this.renderer.clearSelection()
        result = ok()
        break
      case "screenshot": {
        const input = parsed as ParamsOf<"screenshot">
        this.renderer.captureScreenshot(input.path)
        result = { path: input.path }
        break
      }
      case "clockPause":
        result = { nowMs: this.renderer.clockPause() }
        break
      case "clockSet": {
        const input = parsed as ParamsOf<"clockSet">
        result = { nowMs: this.renderer.clockSet(input.nowMs) }
        break
      }
      case "clockFastForward": {
        const input = parsed as ParamsOf<"clockFastForward">
        result = { nowMs: this.renderer.clockFastForward(input.deltaMs) }
        break
      }
      case "clockResume":
        result = { nowMs: this.renderer.clockResume() }
        break
    }

    return methods[method].result.parse(result) as ResultOf<M>
  }

  async close(): Promise<void> {}
}

interface PendingRequest {
  resolve(response: AutomationResponse): void
  reject(error: unknown): void
}

/** Multiplexes typed calls over an SSE text stream. */
export class SseBackend implements AutomationBackend {
  #nextId = 1
  readonly #pending = new Map<number, PendingRequest>()

  constructor(
    readonly write: (chunk: string) => void,
    feed: (listener: (chunk: string) => void) => void,
    readonly onClose?: () => Promise<void>,
  ) {
    const decoder = createSseDecoder((message) => {
      if (!("id" in message) || "method" in message || "event" in message) return
      const pending = this.#pending.get(message.id)
      if (pending === undefined) return
      this.#pending.delete(message.id)
      pending.resolve(parseResponse(message))
    })
    feed((chunk) => decoder.feed(chunk))
  }

  async call<M extends MethodName>(method: M, params: ParamsOf<M>): Promise<ResultOf<M>> {
    methods[method].params.parse(params)
    const id = this.#nextId++
    const response = await new Promise<AutomationResponse>((resolve, reject) => {
      this.#pending.set(id, { resolve, reject })
      this.write(encodeSse({ id, method, params } as AutomationRequest))
    })
    if ("error" in response) {
      throw new AutomationError(response.error.code, response.error.message, response.error.data)
    }
    return methods[method].result.parse(response.result) as ResultOf<M>
  }

  async close(): Promise<void> {
    for (const [id, pending] of this.#pending) {
      pending.reject(new AutomationError("Closed", `Automation request ${id} closed`))
    }
    this.#pending.clear()
    await this.onClose?.()
  }
}

interface Selector {
  testId?: string
  text?: string
  type?: string
  within?: Selector
}

function matches(node: TreeNode, selector: Selector): boolean {
  if (selector.testId !== undefined && node.testId !== selector.testId) return false
  if (selector.type !== undefined && node.type !== selector.type) return false
  if (selector.text !== undefined && !(node.text ?? "").includes(selector.text)) return false
  return true
}

function descendants(root: TreeNode): TreeNode[] {
  return [root, ...(root.children ?? []).flatMap(descendants)]
}

function query(tree: TreeNode | null, selector: Selector): TreeNode[] {
  if (tree === null) return []
  const searchRoots =
    selector.within === undefined
      ? [tree]
      : query(tree, selector.within).flatMap((node) => node.children ?? [])
  return searchRoots.flatMap(descendants).filter((node) => matches(node, selector))
}

function keySequence(text: string): string {
  return [...text]
    .map((character) => {
      if (character === " ") return "space"
      if (character === "\n") return "enter"
      if (character === "\t") return "tab"
      return character
    })
    .join(" ")
}

export class Locator {
  constructor(
    readonly app: App,
    readonly selector: Selector,
  ) {}

  getByTestId(testId: string): Locator {
    return new Locator(this.app, { testId, within: this.selector })
  }

  getByText(text: string): Locator {
    return new Locator(this.app, { text, within: this.selector })
  }

  getByType(type: string): Locator {
    return new Locator(this.app, { type, within: this.selector })
  }

  async all(): Promise<TreeNode[]> {
    return query((await this.app.call("getTree", {})).tree, this.selector)
  }

  async count(): Promise<number> {
    return (await this.all()).length
  }

  async element(): Promise<TreeNode> {
    const elements = await this.all()
    if (elements.length === 0) {
      throw new AutomationError("NotFound", "Locator matched no elements")
    }
    if (elements.length !== 1) {
      throw new AutomationError("Ambiguous", `Locator matched ${elements.length} elements`)
    }
    return elements[0]!
  }

  async bounds(): Promise<ElementBounds> {
    const element = await this.element()
    if (element.bounds !== undefined) return element.bounds
    const result = await this.app.call("getBounds", { elementId: element.id })
    if (result.bounds === null) {
      throw new AutomationError("NotFound", "Element does not have painted bounds")
    }
    return result.bounds
  }

  async click(): Promise<void> {
    const bounds = await this.bounds()
    await this.app.call("click", {
      x: bounds.x + bounds.width / 2,
      y: bounds.y + bounds.height / 2,
    })
  }

  async fill(text: string): Promise<void> {
    const element = await this.element()
    const selectAll = process.platform === "darwin" ? "cmd-a" : "ctrl-a"
    const replacement = text === "" ? "backspace" : keySequence(text)
    await this.app.call("keystrokes", {
      elementId: element.id,
      keys: `${selectAll} ${replacement}`,
    })
  }

  async press(key: string): Promise<void> {
    const element = await this.element()
    await this.app.call("keystrokes", { elementId: element.id, keys: key })
  }

  async textContent(): Promise<string> {
    return (await this.element()).text ?? ""
  }

  async waitFor(options: { timeoutMs?: number } = {}): Promise<TreeNode> {
    const timeoutMs = options.timeoutMs ?? 5000
    const deadline = Date.now() + timeoutMs
    do {
      const elements = await this.all()
      if (elements.length === 1) return elements[0]!
      if (elements.length > 1) {
        throw new AutomationError("Ambiguous", `Locator matched ${elements.length} elements`)
      }
      await new Promise<void>((resolve) => setTimeout(resolve, 16))
    } while (Date.now() < deadline)
    throw new AutomationError("Timeout", `Locator timed out after ${timeoutMs}ms`)
  }
}

export class App {
  readonly clock = {
    pause: async (): Promise<number> => (await this.call("clockPause", {})).nowMs,
    set: async (nowMs: number): Promise<number> => (await this.call("clockSet", { nowMs })).nowMs,
    fastForward: async (deltaMs: number): Promise<number> =>
      (await this.call("clockFastForward", { deltaMs })).nowMs,
    resume: async (): Promise<number> => (await this.call("clockResume", {})).nowMs,
  }

  constructor(readonly backend: AutomationBackend) {}

  call<M extends MethodName>(method: M, params: ParamsOf<M>): Promise<ResultOf<M>> {
    return this.backend.call(method, params)
  }

  getByTestId(testId: string): Locator {
    return new Locator(this, { testId })
  }

  getByText(text: string): Locator {
    return new Locator(this, { text })
  }

  getByType(type: string): Locator {
    return new Locator(this, { type })
  }

  async screenshot(options: { path: string }): Promise<string> {
    return (await this.call("screenshot", { path: options.path })).path
  }

  async captureFrames(directory: string, timesMs: readonly number[]): Promise<string[]> {
    await mkdir(directory, { recursive: true })
    await this.clock.pause()
    const files: string[] = []
    for (const nowMs of timesMs) {
      await this.clock.set(nowMs)
      const file = path.join(directory, `t${nowMs}.png`)
      await this.screenshot({ path: file })
      files.push(file)
    }
    return files
  }

  close(): Promise<void> {
    return this.backend.close()
  }
}

export interface LiveAutomationRenderer {
  simulateClick(x: number, y: number, button?: number): void
  simulateMouseDown(x: number, y: number, button?: number): void
  simulateMouseUp(x: number, y: number, button?: number): void
  simulateMouseMove(x: number, y: number, pressedButton?: number): void
  tick?(): void
  focusElement(elementId: number): void
  blur(): void
  scrollTo(elementId: number, x: number, y: number): void
  getScrollOffset(elementId: number): number[] | null
  getAllText(): string[]
  getPaintedText(): string[]
  getSelectedText(): string | null
  clearSelection(): void
  captureScreenshot(path: string): void
  getAutomationTree(): string
  getElementBounds(elementId: number): number[] | null
  clockPause(): number
  clockSet(nowMs: number): number
  clockFastForward(deltaMs: number): number
  clockResume(): number
}

export function liveRendererAsTest(renderer: LiveAutomationRenderer): TestAutomationRenderer {
  const unsupported = (name: string): never => {
    throw new AutomationError("Unsupported", `${name} is not available for live automation`)
  }
  const afterInput = (): void => renderer.tick?.()
  return {
    nativeSimulateClick(x, y) {
      renderer.simulateClick(x, y)
      afterInput()
    },
    nativeSimulateMouseDown(x, y, button) {
      renderer.simulateMouseDown(x, y, button)
      afterInput()
    },
    nativeSimulateMouseUp(x, y, button) {
      renderer.simulateMouseUp(x, y, button)
      afterInput()
    },
    nativeSimulateMouseMove(x, y, pressedButton) {
      renderer.simulateMouseMove(x, y, pressedButton)
      afterInput()
    },
    nativeSimulateScrollWheel: () => unsupported("scrollWheel"),
    simulateKeystrokes: () => unsupported("keystrokes"),
    nativeSimulateKeystrokes: () => unsupported("keystrokes"),
    nativeSimulateKeyDown: () => unsupported("keyDown"),
    nativeSimulateKeyUp: () => unsupported("keyUp"),
    scrollTo: (id, x, y) => renderer.scrollTo(id, x, y),
    getScrollOffset: (id) => {
      const value = renderer.getScrollOffset(id)
      return value === null ? null : [value[0] ?? 0, value[1] ?? 0]
    },
    getAllText: () => renderer.getAllText(),
    getPaintedText: () => renderer.getPaintedText(),
    getSelectedText: () => renderer.getSelectedText(),
    clearSelection: () => renderer.clearSelection(),
    captureScreenshot: (file) => renderer.captureScreenshot(file),
    getAutomationTree: () => renderer.getAutomationTree(),
    getElementBounds: (id) => renderer.getElementBounds(id),
    clockPause: () => renderer.clockPause(),
    clockSet: (nowMs) => renderer.clockSet(nowMs),
    clockFastForward: (deltaMs) => renderer.clockFastForward(deltaMs),
    clockResume: () => renderer.clockResume(),
    focusElement: (id) => renderer.focusElement(id),
    blur: () => renderer.blur(),
  }
}

async function initialize(backend: AutomationBackend): Promise<App> {
  const app = new App(backend)
  await app.call("initialize", {
    protocolVersion: PROTOCOL_VERSION,
    client: "gpui-vue/automation",
  })
  return app
}

export function connectTest(renderer: TestAutomationRenderer): Promise<App> {
  return initialize(new InProcessBackend(renderer))
}

export function connectStdio(options: {
  write: (chunk: string) => void
  feed: (listener: (chunk: string) => void) => void
  close?: () => Promise<void>
}): Promise<App> {
  return initialize(new SseBackend(options.write, options.feed, options.close))
}

export async function launch(options: {
  command: string
  args?: string[]
  cwd?: string
  env?: NodeJS.ProcessEnv
}): Promise<App> {
  const child: ChildProcessWithoutNullStreams = spawn(options.command, options.args ?? [], {
    cwd: options.cwd,
    env: { ...process.env, ...options.env },
    stdio: ["pipe", "pipe", "pipe"],
  })
  return connectStdio({
    write: (chunk) => child.stdin.write(chunk),
    feed: (listener) =>
      child.stdout.on("data", (buffer: Buffer) => listener(buffer.toString("utf8"))),
    close: async () => {
      child.kill()
    },
  })
}

export async function handleAutomationRequest(
  raw: unknown,
  backend: AutomationBackend,
): Promise<string> {
  const request = parseWireMessage(raw)
  if (!("method" in request)) {
    throw new AutomationError("Protocol", "Automation server expected a request")
  }
  try {
    const result = await backend.call(request.method, request.params as never)
    return encodeSse({ id: request.id, result } as AutomationResponse)
  } catch (error) {
    const failure =
      error instanceof AutomationError ? error : new AutomationError("Protocol", String(error))
    return encodeSse({
      id: request.id,
      error: { code: failure.code, message: failure.message, data: failure.data },
    })
  }
}

export function serveAutomationStdio(backend: AutomationBackend): void {
  const decoder = createSseDecoder((message) => {
    if (!("method" in message)) return
    void handleAutomationRequest(message, backend).then((reply) => process.stdout.write(reply))
  })
  process.stdin.setEncoding("utf8")
  process.stdin.on("data", (chunk: string) => decoder.feed(chunk))
}

export function isServerEvent(message: unknown): message is AutomationServerEvent {
  return (
    message !== null && typeof message === "object" && "event" in message && !("method" in message)
  )
}
