<script lang="ts">
  import { Text } from "../components.js";
  import { selection, COMBOBOX } from "../control-context.svelte.js";
  import type { ComboboxValueProps } from "../control-types.js";
  let { placeholder = "", children, ...props }: ComboboxValueProps = $props();
  const context = selection(COMBOBOX);
  const label = $derived(
    (Array.isArray(context.value) ? context.value : [context.value])
      .filter(Boolean)
      .map((value) => context.items.get(value)?.label ?? value)
      .join(", "),
  );
</script>

<Text {...props}
  >{#if children}{@render children()}{:else}{label || placeholder}{/if}</Text
>
