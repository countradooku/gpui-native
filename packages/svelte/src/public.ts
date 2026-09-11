/** Svelte's client APIs sharing the exact runtime that owns the native components. */
import { api } from "./engine.js"
export const {
  onMount,
  onDestroy,
  getContext,
  setContext,
  hasContext,
  getAllContexts,
  createContext,
  tick,
  settled,
  flushSync,
  untrack,
  getAbortSignal,
} = api
export type { Component, ComponentProps, Snippet } from "svelte"
