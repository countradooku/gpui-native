<script lang="ts">
  import { View } from "../components.js";
  import { selection, COMBOBOX } from "../control-context.svelte.js";
  import type { HostProps } from "../types.js";
  let { children, ...props }: HostProps = $props();
  const context = selection(COMBOBOX);
</script>

{#if context.visible.length === 0}<View {...props}>{@render children?.()}</View
  >{/if}
