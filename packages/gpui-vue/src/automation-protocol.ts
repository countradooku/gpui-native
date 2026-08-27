import { createParser, type EventSourceMessage } from "eventsource-parser"
import { z } from "zod"

export const PROTOCOL_VERSION = 1 as const

export const automationErrorCodes = [
  "Timeout",
  "NotFound",
  "Ambiguous",
  "Protocol",
  "Closed",
  "Unsupported",
  "Security",
  "Cancelled",
] as const

export type AutomationErrorCode = (typeof automationErrorCodes)[number]

export class AutomationError extends Error {
  constructor(
    readonly code: AutomationErrorCode,
    message: string,
    readonly data?: unknown,
  ) {
    super(message)
    this.name = "AutomationError"
  }
}

export const boundsSchema = z.object({
  x: z.number(),
  y: z.number(),
  width: z.number(),
  height: z.number(),
})

export type ElementBounds = z.infer<typeof boundsSchema>

export interface TreeNode {
  id: number
  type: string
  text?: string | undefined
  testId?: string | undefined
  style?: Record<string, unknown> | undefined
  events?: string[] | undefined
  customProps?: Record<string, unknown> | undefined
  bounds?: ElementBounds | undefined
  children?: TreeNode[] | undefined
}

export const treeNodeSchema: z.ZodType<TreeNode> = z.lazy(() =>
  z.object({
    id: z.number(),
    type: z.string(),
    text: z.string().optional(),
    testId: z.string().optional(),
    style: z.record(z.string(), z.unknown()).optional(),
    events: z.array(z.string()).optional(),
    customProps: z.record(z.string(), z.unknown()).optional(),
    bounds: boundsSchema.optional(),
    children: z.array(treeNodeSchema).optional(),
  }),
)

const point = z.object({ x: z.number(), y: z.number() })
const mouseButton = z.number().int().min(0).max(2).optional()
const modifiers = z.string().optional()
const ok = z.object({ ok: z.literal(true) })
const now = z.object({ nowMs: z.number() })

/** Runtime schemas and inferred types share this single method catalog. */
export const methods = {
  initialize: {
    params: z.object({
      protocolVersion: z.literal(PROTOCOL_VERSION),
      client: z.string(),
    }),
    result: z.object({
      protocolVersion: z.literal(PROTOCOL_VERSION),
      pid: z.number().int(),
      capabilities: z.array(z.enum(["input", "screenshot", "clock", "tree"])),
      window: z.object({ width: z.number(), height: z.number() }),
    }),
  },
  cancel: { params: z.object({ id: z.number().int() }), result: ok },
  click: { params: point.extend({ button: mouseButton, modifiers }), result: ok },
  mouseDown: { params: point.extend({ button: mouseButton, modifiers }), result: ok },
  mouseUp: { params: point.extend({ button: mouseButton, modifiers }), result: ok },
  mouseMove: { params: point.extend({ pressedButton: mouseButton, modifiers }), result: ok },
  scrollWheel: {
    params: point.extend({ deltaX: z.number(), deltaY: z.number(), modifiers }),
    result: ok,
  },
  keystrokes: {
    params: z.object({ keys: z.string(), elementId: z.number().optional() }),
    result: ok,
  },
  keyDown: {
    params: z.object({
      key: z.string(),
      isHeld: z.boolean().optional(),
      elementId: z.number().optional(),
    }),
    result: ok,
  },
  keyUp: {
    params: z.object({ key: z.string(), elementId: z.number().optional() }),
    result: ok,
  },
  focus: { params: z.object({ elementId: z.number() }), result: ok },
  blur: { params: z.object({}), result: ok },
  scrollTo: {
    params: z.object({ elementId: z.number(), x: z.number(), y: z.number() }),
    result: ok,
  },
  getScrollOffset: {
    params: z.object({ elementId: z.number() }),
    result: z.object({ offset: z.tuple([z.number(), z.number()]).nullable() }),
  },
  getTree: {
    params: z.object({}),
    result: z.object({ tree: treeNodeSchema.nullable() }),
  },
  getPaintedText: {
    params: z.object({}),
    result: z.object({ text: z.array(z.string()) }),
  },
  getAllText: {
    params: z.object({}),
    result: z.object({ text: z.array(z.string()) }),
  },
  getBounds: {
    params: z.object({ elementId: z.number() }),
    result: z.object({ bounds: boundsSchema.nullable() }),
  },
  getSelectedText: {
    params: z.object({}),
    result: z.object({ text: z.string().nullable() }),
  },
  clearSelection: { params: z.object({}), result: ok },
  screenshot: {
    params: z.object({ path: z.string() }),
    result: z.object({ path: z.string() }),
  },
  clockPause: { params: z.object({}), result: now },
  clockSet: {
    params: z.object({ nowMs: z.number().nonnegative() }),
    result: now,
  },
  clockFastForward: {
    params: z.object({ deltaMs: z.number().nonnegative() }),
    result: now,
  },
  clockResume: { params: z.object({}), result: now },
} as const

export type MethodName = keyof typeof methods
export type ParamsOf<M extends MethodName> = z.infer<(typeof methods)[M]["params"]>
export type ResultOf<M extends MethodName> = z.infer<(typeof methods)[M]["result"]>

export type AutomationRequest<M extends MethodName = MethodName> = {
  [K in M]: { id: number; method: K; params: ParamsOf<K> }
}[M]

export type AutomationSuccess<M extends MethodName = MethodName> = {
  [K in M]: { id: number; result: ResultOf<K> }
}[M]

export interface AutomationFailure {
  id: number
  error: { code: AutomationErrorCode; message: string; data?: unknown }
}

export type AutomationResponse<M extends MethodName = MethodName> =
  | AutomationSuccess<M>
  | AutomationFailure

export type AutomationServerEvent =
  | { event: "console"; params: { text: string } }
  | { event: "frame"; params: { n: number; path?: string | undefined } }
  | { event: "closed"; params: { reason: string } }

export type WireMessage = AutomationRequest | AutomationResponse | AutomationServerEvent

const errorCodeSchema = z.enum(automationErrorCodes)
const serverEventSchema = z.discriminatedUnion("event", [
  z.object({ event: z.literal("console"), params: z.object({ text: z.string() }) }),
  z.object({
    event: z.literal("frame"),
    params: z.object({ n: z.number(), path: z.string().optional() }),
  }),
  z.object({ event: z.literal("closed"), params: z.object({ reason: z.string() }) }),
])

function record(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new AutomationError("Protocol", "Automation message must be an object")
  }
  return value as Record<string, unknown>
}

function methodName(value: unknown): MethodName {
  if (typeof value !== "string" || !(value in methods)) {
    throw new AutomationError("Protocol", `Unknown automation method: ${String(value)}`)
  }
  return value as MethodName
}

function messageId(value: unknown): number {
  if (typeof value !== "number" || !Number.isInteger(value)) {
    throw new AutomationError("Protocol", "Automation message id must be an integer")
  }
  return value
}

export function parseRequest(value: unknown): AutomationRequest {
  const input = record(value)
  const method = methodName(input.method)
  const params = methods[method].params.safeParse(input.params)
  if (!params.success) {
    throw new AutomationError("Protocol", `Invalid ${method} parameters`, params.error.flatten())
  }
  return { id: messageId(input.id), method, params: params.data } as AutomationRequest
}

export function parseResponse(value: unknown): AutomationResponse {
  const input = record(value)
  const id = messageId(input.id)
  if ("error" in input) {
    const parsed = z
      .object({ code: errorCodeSchema, message: z.string(), data: z.unknown().optional() })
      .safeParse(input.error)
    if (!parsed.success) {
      throw new AutomationError("Protocol", "Invalid automation error response")
    }
    return { id, error: parsed.data }
  }
  if (!("result" in input)) {
    throw new AutomationError("Protocol", "Automation response has no result or error")
  }
  return { id, result: input.result } as AutomationResponse
}

export function parseServerEvent(value: unknown): AutomationServerEvent {
  const parsed = serverEventSchema.safeParse(value)
  if (!parsed.success) {
    throw new AutomationError("Protocol", "Invalid automation server event")
  }
  return parsed.data
}

export function parseWireMessage(value: unknown): WireMessage {
  const input = record(value)
  if ("method" in input) return parseRequest(input)
  if ("event" in input) return parseServerEvent(input)
  return parseResponse(input)
}

export function encodeSse(message: WireMessage): string {
  return `data: ${JSON.stringify(message)}\n\n`
}

export interface SseDecoder {
  feed(chunk: string): void
  reset(): void
}

export function createSseDecoder(onMessage: (message: WireMessage) => void): SseDecoder {
  const onEvent = (event: EventSourceMessage): void => {
    try {
      onMessage(parseWireMessage(JSON.parse(event.data) as unknown))
    } catch (error) {
      if (error instanceof AutomationError) throw error
      throw new AutomationError("Protocol", `Invalid automation JSON: ${String(error)}`)
    }
  }
  const parser = createParser({ onEvent })
  return {
    feed: (chunk) => parser.feed(chunk),
    reset: () => parser.reset(),
  }
}

export function decodeSseChunk(chunk: string): WireMessage[] {
  const messages: WireMessage[] = []
  createSseDecoder((message) => messages.push(message)).feed(chunk)
  return messages
}
