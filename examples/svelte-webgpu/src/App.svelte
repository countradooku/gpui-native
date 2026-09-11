<script lang="ts">
  import { Canvas, Column, Text, useGPUCanvas } from "@gpui-native/svelte";
  import { startWebGPUScene } from "../../shared/webgpu-scene.js";
  let { gpu }: { gpu: GPU } = $props();
  const canvas = useGPUCanvas(() => ({ width: 640, height: 400 }));
  let error = $state("");
  $effect(() => {
    const current = canvas.current;
    if (!current) return;
    let cancelled = false,
      stop: (() => void) | undefined;
    async function start() {
      try {
        const dispose = await startWebGPUScene(
          current!,
          gpu,
          (next) => (error = String(next)),
        );
        if (cancelled) dispose();
        else stop = dispose;
      } catch (next) {
        if (!cancelled) error = String(next);
      }
    }
    void start();
    return () => {
      cancelled = true;
      stop?.();
    };
  });
</script>

<Column
  style={{
    width: "100%",
    height: "100%",
    padding: 24,
    gap: 16,
    background: "#111827",
    color: "#f8fafc",
  }}
  ><Text>Svelte runes + WebGPU</Text>{#if canvas.current}<Canvas
      source={canvas.current.id}
      style={{ width: 640, height: 400 }}
    />{/if}{#if error}<Text>{error}</Text>{/if}</Column
>
