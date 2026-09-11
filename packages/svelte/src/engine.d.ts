import type { HostNode } from "./host.js"
export const api: typeof import("svelte")
export const stores: typeof import("svelte/store")
export const reactivity: typeof import("svelte/reactivity")
export function render_effect(fn: () => void | (() => void)): unknown
export function effect_root(fn: () => void): () => void
export function assign_nodes(start: HostNode, end: HostNode | null): void
export function push(props: object, runes?: boolean, component?: Function): void
export function pop<T>(exports?: T): T
export function flush<T>(fn?: () => T): T
export function get<T>(source: { v: T }): T
export function set<T>(source: { v: T }, value: T): T
export function state<T>(value: T): { v: T }
export function untrack<T>(fn: () => T): T
export function snippet(
  anchor: HostNode,
  getSnippet: () => ((anchor: HostNode, ...args: unknown[]) => void) | null | undefined,
  ...args: (() => unknown)[]
): void
export function snapshot<T>(value: T): T
export function proxy<T extends object>(value: T): T
