<script lang="ts">
  import { Column, Text } from "../components.js";
  import type { Component } from "svelte";
  let {
    component: App,
    props,
    error,
    onError,
  }: {
    component: Component<Record<string, unknown>>;
    props: Record<string, unknown>;
    error?: unknown;
    onError?: (error: unknown) => void;
  } = $props();
</script>

{#snippet failure(error: unknown)}
  <Column
    role="alert"
    aria-label="Application runtime error"
    style={{
      width: "100%",
      height: "100%",
      padding: 20,
      background: "#240e16",
      color: "#ffccd5",
      overflow: "scroll",
    }}
    ><Text>Application runtime error</Text><Text
      >{error instanceof Error
        ? (error.stack ?? error.message)
        : String(error)}</Text
    ></Column
  >
{/snippet}
{#if error !== undefined}{@render failure(error)}{:else}
  <svelte:boundary onerror={(error) => onError?.(error)}>
    <App {...props} />
    {#snippet failed(error: unknown)}{@render failure(error)}{/snippet}
  </svelte:boundary>
{/if}
