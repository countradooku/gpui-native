<script lang="ts">
  import { TextInput } from "../components.js";
  import { selection, COMBOBOX } from "../control-context.svelte.js";
  import type { ComboboxInputProps } from "../control-types.js";
  import type { ElementRef } from "../types.js";
  let { ...props }: ComboboxInputProps = $props();
  const context = selection(COMBOBOX);
  let ref = $state.raw<ElementRef | null>(null);
  $effect(() => {
    context.setTrigger(ref);
    return () => context.setTrigger(null);
  });
</script>

<TextInput
  {...props}
  bind:ref
  role="combobox"
  aria-expanded={context.open}
  value={context.inputValue}
  readOnly={context.disabled || props.readOnly || false}
  onChange={(event) => {
    props.onChange?.(event);
    context.setInput(event.value ?? "");
  }}
  onKeyDown={(event) => {
    props.onKeyDown?.(event);
    context.key(event);
  }}
  onFocus={(event) => {
    props.onFocus?.(event);
    context.setOpen(true);
  }}
/>
