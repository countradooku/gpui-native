<script lang="ts">
  import { View } from "../components.js";
  import type { HostProps } from "../types.js";
  let { children, ...props }: HostProps = $props();
</script>

<View {...props} role="separator">{@render children?.()}</View>
