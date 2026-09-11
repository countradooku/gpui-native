<script lang="ts">
  import { View } from "../components.js";
  import { selection, COMBOBOX } from "../control-context.svelte.js";
  import type { ComboboxListProps } from "../control-types.js";
  let { children, item, ...props }: ComboboxListProps = $props();
  const context = selection(COMBOBOX);
</script>

<View
  {...props}
  role="listbox"
  onKeyDown={(event) => {
    props.onKeyDown?.(event);
    context.key(event);
  }}
>
  {#if item}{#each context.visible as entry, index (entry.value)}{@render item(
        entry.value,
        index,
      )}{/each}{:else}{@render children?.()}{/if}
</View>
