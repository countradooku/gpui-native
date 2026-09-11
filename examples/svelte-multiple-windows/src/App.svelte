<script lang="ts">
  import {
    Button,
    Column,
    Text,
    createWindow,
    type GpuiWindowRoot,
  } from "@gpui-native/svelte";
  import { onDestroy } from "svelte";
  import Inspector from "./Inspector.svelte";
  let count = $state(0);
  const windows: GpuiWindowRoot[] = [];
  function open() {
    count++;
    windows.push(
      createWindow(Inspector, {
        props: { number: count },
        title: `Svelte inspector ${count}`,
        width: 420,
        height: 280,
      }),
    );
  }
  onDestroy(() => windows.forEach((window) => window.close()));
</script>

<Column
  style={{
    width: "100%",
    height: "100%",
    padding: 32,
    gap: 16,
    background: "#111827",
    color: "#f8fafc",
  }}
  ><Text style={{ fontSize: 28 }}>Independent native windows</Text><Button
    onPress={open}
    style={{ padding: 14, background: "#30466e", borderRadius: 8 }}
    >Open inspector</Button
  ><Text>Created {count} windows</Text></Column
>
