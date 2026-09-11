<script lang="ts">
  import { Anchored, View } from "../components.js";
  import type { FloatingContentProps } from "../control-types.js";
  let {
    side = "bottom",
    sideOffset = 0,
    align = "start",
    alignOffset = 0,
    collisionPadding = 8,
    contentRef = $bindable(null),
    children,
    ...props
  }: FloatingContentProps = $props();
</script>

<Anchored
  {side}
  {align}
  gap={sideOffset}
  offset={side === "top" || side === "bottom"
    ? { x: alignOffset, y: 0 }
    : { x: 0, y: alignOffset }}
  fit="snap"
  snapMargin={collisionPadding}
  deferred
  priority={1}
  occlude
>
  <View {...props} bind:ref={contentRef}>{@render children?.()}</View>
</Anchored>
