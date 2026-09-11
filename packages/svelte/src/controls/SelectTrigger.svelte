<script lang="ts">
  import { View } from "../components.js";
  import { selection, SELECT } from "../control-context.svelte.js";
  import type { SelectTriggerProps } from "../control-types.js";
  import type { ElementRef, HostProps } from "../types.js";
  let { child, children, ...props }: SelectTriggerProps = $props();
  const context = selection(SELECT);
  let ref = $state.raw<ElementRef | null>(null);
  $effect(() => {
    context.setTrigger(ref);
    return () => context.setTrigger(null);
  });
  const host = $derived({
    ...props,
    get ref() {
      return ref;
    },
    set ref(next: ElementRef | null) {
      ref = next ?? null;
    },
    role: "combobox",
    "aria-expanded": context.open,
    tabIndex: context.disabled ? -1 : (props.tabIndex ?? 0),
    onClick: (event) => {
      props.onClick?.(event);
      context.setOpen(!context.open);
    },
    onKeyDown: (event) => {
      props.onKeyDown?.(event);
      context.key(event);
    },
  } satisfies HostProps);
</script>

{#if child}{@render child(host)}{:else}<View {...host} bind:ref
    >{@render children?.()}</View
  >{/if}
