<script lang="ts">
  import FloatingLayer from "./FloatingLayer.svelte";
  import { tooltip } from "../control-context.svelte.js";
  import type { TooltipContentProps } from "../control-types.js";
  let { children, ...props }: TooltipContentProps = $props();
  const context = tooltip();
</script>

{#if context.open}<FloatingLayer
    {...props}
    role="tooltip"
    onMouseEnter={(event) => {
      props.onMouseEnter?.(event);
      context.show(true);
    }}
    onMouseLeave={(event) => {
      props.onMouseLeave?.(event);
      context.hide();
    }}
    onMouseDownOutside={(event) => {
      props.onMouseDownOutside?.(event);
      context.hide();
    }}>{@render children?.()}</FloatingLayer
  >{/if}
