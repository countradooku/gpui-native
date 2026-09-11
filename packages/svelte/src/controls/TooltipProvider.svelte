<script lang="ts">
  import { setContext } from "../public.js";
  import { TOOLTIP_PROVIDER } from "../control-context.svelte.js";
  import type { TooltipProviderProps } from "../control-types.js";
  let {
    delayDuration = 700,
    skipDelayDuration = 300,
    children,
  }: TooltipProviderProps = $props();
  let closedAt = 0;
  setContext(TOOLTIP_PROVIDER, {
    get delay() {
      return performance.now() - closedAt < skipDelayDuration
        ? 0
        : delayDuration;
    },
    closed() {
      closedAt = performance.now();
    },
  });
</script>

{@render children?.()}
