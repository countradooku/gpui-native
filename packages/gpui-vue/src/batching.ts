import type { NativeNodeId, NativeRenderer } from "./native.js"

type MutationArgument = string | number | boolean | object | null
type Mutation = [string, ...MutationArgument[]]

const BATCHED_METHODS = new Set([
  "createElement",
  "appendChild",
  "removeChild",
  "insertBefore",
  "setStyle",
  "setText",
  "setEventListener",
  "setRoot",
  "setCustomProp",
])

export interface BatchingRenderer extends NativeRenderer {
  readonly innerRenderer: NativeRenderer
  flushMutations(): NativeNodeId[]
}

/**
 * Coalesces Vue's host operations into one native call per microtask. Vue does
 * not expose a renderer commit hook, so scheduling the flush after the first
 * mutation naturally places it after Vue's synchronous patch wave.
 */
export function wrapWithBatching(inner: NativeRenderer): BatchingRenderer {
  let queue: Mutation[] = []
  let scheduled = false

  const flushMutations = (): NativeNodeId[] => {
    scheduled = false
    if (queue.length === 0) return []

    const pending = queue
    let destroyed: NativeNodeId[] = []
    if (typeof inner.applyBatch === "function") {
      destroyed = inner.applyBatch(JSON.stringify(pending))
    } else {
      for (const [operation, ...args] of pending) {
        replayMutation(inner, operation, args)
      }
      inner.commitMutations()
    }
    queue = []
    return destroyed
  }

  const scheduleFlush = (): void => {
    if (scheduled) return
    scheduled = true
    queueMicrotask(flushMutations)
  }

  const enqueue = (operation: string, args: MutationArgument[]): void => {
    queue.push([operation, ...args])
    scheduleFlush()
  }

  return new Proxy(inner, {
    get(target, property, receiver): unknown {
      if (property === "innerRenderer") return inner
      if (property === "flushMutations" || property === "commitMutations") {
        return flushMutations
      }

      if (property === "destroyElement") {
        return (id: NativeNodeId): NativeNodeId[] => {
          enqueue("destroyElement", [id])
          return []
        }
      }

      if (typeof property === "string" && BATCHED_METHODS.has(property)) {
        return (...args: MutationArgument[]): void => {
          const operation = property === "setCustomProp" ? "setCustomPropValue" : property
          enqueue(operation, args)
        }
      }

      const value: unknown = Reflect.get(target, property, receiver)
      if (typeof value !== "function") return value

      return (...args: unknown[]): unknown => {
        flushMutations()
        return Reflect.apply(value, target, args)
      }
    },
  }) as BatchingRenderer
}

function replayMutation(
  renderer: NativeRenderer,
  operation: string,
  args: MutationArgument[],
): void {
  switch (operation) {
    case "createElement":
      renderer.createElement(Number(args[0]), String(args[1]))
      break
    case "destroyElement":
      renderer.destroyElement(Number(args[0]))
      break
    case "appendChild":
      renderer.appendChild(Number(args[0]), Number(args[1]))
      break
    case "removeChild":
      renderer.removeChild(Number(args[0]), Number(args[1]))
      break
    case "insertBefore":
      renderer.insertBefore(Number(args[0]), Number(args[1]), Number(args[2]))
      break
    case "setStyle":
      renderer.setStyle(Number(args[0]), JSON.stringify(args[1] ?? {}))
      break
    case "setText":
      renderer.setText(Number(args[0]), String(args[1]))
      break
    case "setEventListener":
      renderer.setEventListener(Number(args[0]), String(args[1]), Boolean(args[2]))
      break
    case "setRoot":
      renderer.setRoot(Number(args[0]))
      break
    case "setCustomPropValue":
      renderer.setCustomProp(Number(args[0]), String(args[1]), JSON.stringify(args[2] ?? null))
      break
    default:
      throw new Error(`Unknown gpui-vue mutation: ${operation}`)
  }
}
