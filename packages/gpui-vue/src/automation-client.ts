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

function importNodeModule<T>(specifier: string): Promise<T> {
  return import(specifier)
}

export interface AutomationBackend {
  call<M extends MethodName>(method: M, params: ParamsOf<M>): Promise<ResultOf<M>>
  close(): Promise<void>
}

export interface TestAutomationRenderer {
  nativeSimulateClick(x: number, y: number, button?: number, modifiers?: string): void
  nativeSimulateMouseDown(x: number, y: number, button?: number, modifiers?: string): void
  nativeSimulateMouseUp(x: number, y: number, button?: number, modifiers?: string): void
  nativeSimulateMouseMove(x: number, y: number, pressedButton?: number, modifiers?: string): void
  nativeSimulateScrollWheel(
    x: number,
    y: number,
    deltaX: number,
    deltaY: number,
    modifiers?: string,
  ): void
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
          pid: typeof process === "undefined" ? 0 : process.pid,
          capabilities:
            typeof window === "undefined"
              ? ["input", "screenshot", "clock", "tree"]
              : ["input", "clock", "tree"],
          window: { width: 800, height: 600 },
        }
        break
      case "cancel":
        result = ok()
        break
      case "click": {
        const input = parsed as ParamsOf<"click">
        this.renderer.nativeSimulateClick(input.x, input.y, input.button, input.modifiers)
        result = ok()
        break
      }
      case "mouseDown": {
        const input = parsed as ParamsOf<"mouseDown">
        this.renderer.nativeSimulateMouseDown(input.x, input.y, input.button, input.modifiers)
        result = ok()
        break
      }
      case "mouseUp": {
        const input = parsed as ParamsOf<"mouseUp">
        this.renderer.nativeSimulateMouseUp(input.x, input.y, input.button, input.modifiers)
        result = ok()
        break
      }
      case "mouseMove": {
        const input = parsed as ParamsOf<"mouseMove">
        this.renderer.nativeSimulateMouseMove(
          input.x,
          input.y,
          input.pressedButton,
          input.modifiers,
        )
        result = ok()
        break
      }
      case "scrollWheel": {
        const input = parsed as ParamsOf<"scrollWheel">
        this.renderer.nativeSimulateScrollWheel(
          input.x,
          input.y,
          input.deltaX,
          input.deltaY,
          input.modifiers,
        )
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

/** A window-space point, or a locator resolved to the centre of its bounds. */
export type PointTarget = { x: number; y: number } | Locator

export interface MouseOptions {
  button?: number
  /** Held modifiers in `press()` syntax, for example `"cmd-shift"`. */
  modifiers?: string
}

export interface DragOptions extends MouseOptions {
  steps?: number
  offset?: { x: number; y: number }
}

function centerOf(bounds: ElementBounds): { x: number; y: number } {
  return { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 }
}

function collectText(node: TreeNode): string {
  return (node.text ?? "") + (node.children ?? []).map(collectText).join("")
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

  async center(): Promise<{ x: number; y: number }> {
    return centerOf(await this.bounds())
  }

  async click(options: MouseOptions = {}): Promise<void> {
    await this.app.mouse.click(this, options)
  }

  async hover(options: MouseOptions = {}): Promise<void> {
    await this.app.mouse.move(this, options)
  }

  async wheel(deltaX: number, deltaY: number, options: MouseOptions = {}): Promise<void> {
    await this.app.mouse.wheel(this, deltaX, deltaY, options)
  }

  async dragTo(target: PointTarget, options: DragOptions = {}): Promise<void> {
    await this.app.mouse.drag(this, target, options)
  }

  async dragBy(dx: number, dy: number, options: DragOptions = {}): Promise<void> {
    const start = await this.center()
    const offset = options.offset ?? { x: 0, y: 0 }
    await this.app.mouse.drag(
      this,
      { x: start.x + offset.x + dx, y: start.y + offset.y + dy },
      options,
    )
  }

  async fill(text: string): Promise<void> {
    const element = await this.element()
    const browserPlatform = typeof navigator === "undefined" ? "" : navigator.platform
    const selectAll =
      browserPlatform.includes("Mac") ||
      (typeof process !== "undefined" && process.platform === "darwin")
        ? "cmd-a"
        : "ctrl-a"
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
    return collectText(await this.element())
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
  readonly clock: {
    pause: () => Promise<number>
    set: (nowMs: number) => Promise<number>
    fastForward: (deltaMs: number) => Promise<number>
    resume: () => Promise<number>
  }

  readonly mouse: {
    move: (
      target: PointTarget,
      options?: MouseOptions & { pressedButton?: number },
    ) => Promise<void>
    down: (target: PointTarget, options?: MouseOptions) => Promise<void>
    up: (target: PointTarget, options?: MouseOptions) => Promise<void>
    click: (target: PointTarget, options?: MouseOptions) => Promise<void>
    wheel: (
      target: PointTarget,
      deltaX: number,
      deltaY: number,
      options?: MouseOptions,
    ) => Promise<void>
    drag: (from: PointTarget, to: PointTarget, options?: DragOptions) => Promise<void>
  }

  constructor(readonly backend: AutomationBackend) {
    this.mouse = {
      move: async (target, options = {}) => {
        const point = await this.#resolvePoint(target)
        await this.call("mouseMove", {
          ...point,
          pressedButton: options.pressedButton,
          modifiers: options.modifiers,
        })
      },
      down: async (target, options = {}) => {
        await this.call("mouseDown", { ...(await this.#resolvePoint(target)), ...options })
      },
      up: async (target, options = {}) => {
        await this.call("mouseUp", { ...(await this.#resolvePoint(target)), ...options })
      },
      click: async (target, options = {}) => {
        await this.call("click", { ...(await this.#resolvePoint(target)), ...options })
      },
      wheel: async (target, deltaX, deltaY, options = {}) => {
        await this.call("scrollWheel", {
          ...(await this.#resolvePoint(target)),
          deltaX,
          deltaY,
          modifiers: options.modifiers,
        })
      },
      drag: async (from, to, options = {}) => {
        const offset = options.offset ?? { x: 0, y: 0 }
        const origin = await this.#resolvePoint(from)
        const start = { x: origin.x + offset.x, y: origin.y + offset.y }
        const end = await this.#resolvePoint(to)
        const button = options.button ?? 0
        const modifiers = options.modifiers
        const steps = Math.max(1, Math.floor(options.steps ?? 8))
        await this.call("mouseMove", { ...start, modifiers })
        await this.call("mouseDown", { ...start, button, modifiers })
        for (let step = 1; step <= steps; step += 1) {
          const progress = step / steps
          await this.call("mouseMove", {
            x: start.x + (end.x - start.x) * progress,
            y: start.y + (end.y - start.y) * progress,
            pressedButton: button,
            modifiers,
          })
        }
        await this.call("mouseUp", { ...end, button, modifiers })
      },
    }
    this.clock = {
      pause: async () => (await this.call("clockPause", {})).nowMs,
      set: async (nowMs) => (await this.call("clockSet", { nowMs })).nowMs,
      fastForward: async (deltaMs) => (await this.call("clockFastForward", { deltaMs })).nowMs,
      resume: async () => (await this.call("clockResume", {})).nowMs,
    }
  }

  async #resolvePoint(target: PointTarget): Promise<{ x: number; y: number }> {
    return target instanceof Locator ? target.center() : target
  }

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
    if (typeof process === "undefined") {
      throw new AutomationError("Unsupported", "Browser frame capture uses browser automation")
    }
    const { mkdir } = await importNodeModule<typeof import("node:fs/promises")>("node:fs/promises")
    const path = await importNodeModule<typeof import("node:path")>("node:path")
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
  simulateClick(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseDown(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseUp(x: number, y: number, button?: number, modifiers?: string): void
  simulateMouseMove(x: number, y: number, pressedButton?: number, modifiers?: string): void
  simulateScrollWheel(
    x: number,
    y: number,
    deltaX: number,
    deltaY: number,
    modifiers?: string,
  ): void
  simulateKeystrokes?(keystrokes: string): void
  simulateKeyDown?(keystroke: string, isHeld?: boolean): void
  simulateKeyUp?(keystroke: string): void
  tick?(): void
  focusElement(elementId: number): void
  blur(): void
  scrollTo(elementId: number, x: number, y: number): void
  getScrollOffset(elementId: number): number[] | null
  getAllText(): string[]
  getPaintedText(): string[]
  getSelectedText(): string | null
  clearSelection(): void
  captureScreenshot?(path: string): void
  getAutomationTree(): string
  getElementBounds(elementId: number): number[] | null
  clockPause(): number
  clockSet(nowMs: number): number
  clockFastForward(deltaMs: number): number
  clockResume(): number
}

export function liveRendererAsTest(renderer: LiveAutomationRenderer): TestAutomationRenderer {
  const afterInput = (): void => renderer.tick?.()
  return {
    nativeSimulateClick(x, y, button, modifiers) {
      renderer.simulateClick(x, y, button, modifiers)
      afterInput()
    },
    nativeSimulateMouseDown(x, y, button, modifiers) {
      renderer.simulateMouseDown(x, y, button, modifiers)
      afterInput()
    },
    nativeSimulateMouseUp(x, y, button, modifiers) {
      renderer.simulateMouseUp(x, y, button, modifiers)
      afterInput()
    },
    nativeSimulateMouseMove(x, y, pressedButton, modifiers) {
      renderer.simulateMouseMove(x, y, pressedButton, modifiers)
      afterInput()
    },
    nativeSimulateScrollWheel(x, y, deltaX, deltaY, modifiers) {
      renderer.simulateScrollWheel(x, y, deltaX, deltaY, modifiers)
      afterInput()
    },
    simulateKeystrokes(keys) {
      if (renderer.simulateKeystrokes === undefined) {
        throw new AutomationError("Unsupported", "keystrokes are not live yet")
      }
      renderer.simulateKeystrokes(keys)
      afterInput()
    },
    nativeSimulateKeystrokes(elementId, keys) {
      if (renderer.simulateKeystrokes === undefined) {
        throw new AutomationError("Unsupported", "keystrokes are not live yet")
      }
      renderer.focusElement(elementId)
      renderer.simulateKeystrokes(keys)
      afterInput()
    },
    nativeSimulateKeyDown(elementId, key, isHeld) {
      if (renderer.simulateKeyDown === undefined) {
        throw new AutomationError("Unsupported", "keyDown is not live yet")
      }
      if (elementId > 0) renderer.focusElement(elementId)
      renderer.simulateKeyDown(key, isHeld)
      afterInput()
    },
    nativeSimulateKeyUp(elementId, key) {
      if (renderer.simulateKeyUp === undefined) {
        throw new AutomationError("Unsupported", "keyUp is not live yet")
      }
      if (elementId > 0) renderer.focusElement(elementId)
      renderer.simulateKeyUp(key)
      afterInput()
    },
    scrollTo: (id, x, y) => renderer.scrollTo(id, x, y),
    getScrollOffset: (id) => {
      const value = renderer.getScrollOffset(id)
      return value === null ? null : [value[0] ?? 0, value[1] ?? 0]
    },
    getAllText: () => renderer.getAllText(),
    getPaintedText: () => renderer.getPaintedText(),
    getSelectedText: () => renderer.getSelectedText(),
    clearSelection: () => renderer.clearSelection(),
    captureScreenshot(file) {
      if (renderer.captureScreenshot === undefined) {
        throw new AutomationError("Unsupported", "Browser screenshots use browser automation")
      }
      renderer.captureScreenshot(file)
    },
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
  const { spawn } =
    await importNodeModule<typeof import("node:child_process")>("node:child_process")
  const child = spawn(options.command, options.args ?? [], {
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
