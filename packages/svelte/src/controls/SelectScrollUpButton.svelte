<script lang="ts">
  import { Button } from "../components.js";
  import { selection, SELECT } from "../control-context.svelte.js";
  import type { ButtonProps } from "../types.js";
  let { children, ...props }: ButtonProps = $props();
  const context = selection(SELECT);
</script>

<Button
  {...props}
  onPress={(event) => {
    props.onPress?.(event);
    context.key({ ...event, key: "up" });
  }}>{@render children?.()}</Button
>
