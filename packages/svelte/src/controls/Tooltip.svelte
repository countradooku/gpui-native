<script lang="ts">
  import { View } from "../components.js";
  import { getContext, setContext, onDestroy } from "../public.js";
  import { TOOLTIP, TOOLTIP_PROVIDER } from "../control-context.svelte.js";
  import type { TooltipProps } from "../control-types.js";
  let {
    defaultOpen = false,
    open = $bindable(defaultOpen),
    delayDuration,
    onOpenChange,
    children,
    ...props
  }: TooltipProps = $props();
  const provider = getContext<{ delay: number; closed(): void } | undefined>(
    TOOLTIP_PROVIDER,
  );
  let timer: ReturnType<typeof setTimeout> | undefined;
  function clear() {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
  }
  function setOpen(next: boolean) {
    if (open !== next) {
      open = next;
      onOpenChange?.(next);
      if (!next) provider?.closed();
    }
  }
  function hide() {
    clear();
    setOpen(false);
  }
  function show(immediate = false) {
    clear();
    const delay = immediate ? 0 : (delayDuration ?? provider?.delay ?? 700);
    if (delay === 0) setOpen(true);
    else
      timer = setTimeout(() => {
        timer = undefined;
        setOpen(true);
      }, delay);
  }
  onDestroy(clear);
  setContext(TOOLTIP, {
    get open() {
      return open;
    },
    show,
    hide,
  });
</script>

<View {...props} style={{ position: "relative", ...props.style }}
  >{@render children?.()}</View
>
