<script lang="ts">
  import FloatingLayer from "./FloatingLayer.svelte";
  import { selection, SELECT } from "../control-context.svelte.js";
  import type { ElementRef } from "../types.js";
  import type { SelectContentProps } from "../control-types.js";
  let { children, ...props }: SelectContentProps = $props();
  const context = selection(SELECT);
  let content = $state.raw<ElementRef | null>(null);
  $effect(() => {
    context.setContent(content);
    return () => context.setContent(null);
  });
</script>

<FloatingLayer
  {...props}
  bind:contentRef={content}
  role="listbox"
  style={{
    flexDirection: "column",
    ...props.style,
    ...(!context.open ? { display: "none" } : {}),
  }}
  onKeyDown={(event) => {
    props.onKeyDown?.(event);
    context.key(event);
  }}
  onMouseDownOutside={(event) => {
    props.onMouseDownOutside?.(event);
    context.setOpen(false);
  }}>{@render children?.()}</FloatingLayer
>
