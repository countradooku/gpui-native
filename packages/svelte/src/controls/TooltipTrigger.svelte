<script lang="ts">
  import { View } from "../components.js";
  import { tooltip } from "../control-context.svelte.js";
  import type { TooltipTriggerProps } from "../control-types.js";
  import type { HostProps } from "../types.js";
  let { child, children, ...props }: TooltipTriggerProps = $props();
  const context = tooltip();
  const host = $derived({
    ...props,
    tabIndex: props.tabIndex ?? 0,
    onMouseEnter: (event) => {
      props.onMouseEnter?.(event);
      context.show();
    },
    onMouseLeave: (event) => {
      props.onMouseLeave?.(event);
      context.hide();
    },
    onFocus: (event) => {
      props.onFocus?.(event);
      context.show(true);
    },
    onBlur: (event) => {
      props.onBlur?.(event);
      context.hide();
    },
    onClick: (event) => {
      props.onClick?.(event);
      context.hide();
    },
    onKeyDown: (event) => {
      props.onKeyDown?.(event);
      if (event.key?.toLowerCase() === "escape") context.hide();
    },
  } satisfies HostProps);
</script>

{#if child}{@render child(host)}{:else}<View {...host}
    >{@render children?.()}</View
  >{/if}
