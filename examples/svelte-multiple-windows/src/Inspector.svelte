<script lang="ts">
  import { Button, Column, Text, useGpuiWindow } from "@gpui-native/svelte";
  let { number }: { number: number } = $props();
  const window = useGpuiWindow();
</script>

<Column
  style={{
    width: "100%",
    height: "100%",
    padding: 24,
    gap: 16,
    background: "#182030",
    color: "#f8fafc",
  }}
  ><Text>Independent Svelte window {number}</Text><Button
    onPress={() => window.setTitle(`Inspector ${number}`)}
    style={{ padding: 12 }}>Rename window</Button
  ></Column
>
