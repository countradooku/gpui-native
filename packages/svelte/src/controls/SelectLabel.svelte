<script lang="ts">
  import { Text } from "../components.js";
  import type { HostProps } from "../types.js";
  let { children, ...props }: HostProps = $props();
</script>

<Text {...props}>{@render children?.()}</Text>
