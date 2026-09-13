<script lang="ts">
  import {
    Canvas,
    Column,
    Text,
    useGPUCanvas,
    useWindowSize,
  } from "@gpui-native/svelte";
  import { startWebGPUScene } from "../../shared/webgpu-scene.js";
  let { gpu }: { gpu: GPU } = $props();
  const size = useWindowSize();
  const width = $derived(Math.max(1, Math.min(640, size.width - 48)));
  const height = $derived(Math.round((width * 400) / 640));
  const canvas = useGPUCanvas(() => ({ width, height }));
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
    overflow: "scroll",
    padding: 24,
    gap: 16,
    background: "#111827",
    color: "#f8fafc",
  }}
  ><Text>Svelte runes + WebGPU</Text>{#if canvas.current}<Canvas
      source={canvas.current.id}
      style={{ width, height, flexShrink: 0 }}
    />{/if}{#if error}<Text>{error}</Text>{/if}</Column
>
