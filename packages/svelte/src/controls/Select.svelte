<script lang="ts">
  import { View } from "../components.js";
  import { setContext } from "../public.js";
  import { SvelteMap } from "../reactivity.js";
  import { useGpuiRequired } from "../context.svelte.js";
  import { SELECT } from "../control-context.svelte.js";
  import type {
    SelectProps,
    SelectionContext,
    SelectionItem,
  } from "../control-types.js";
  import type { ElementRef, EventPayload } from "../types.js";
  let {
    defaultValue = "",
    value = $bindable(defaultValue),
    defaultOpen = false,
    open = $bindable(defaultOpen),
    disabled = false,
    onValueChange,
    onOpenChange,
    children,
    ...props
  }: SelectProps = $props();
  let active = $state<string | null>(null),
    trigger = $state.raw<ElementRef | null>(null),
    content = $state.raw<ElementRef | null>(null);
  const items = new SvelteMap<string, SelectionItem>(),
    renderer = useGpuiRequired();
  let query = "",
    at = 0;
  function setOpen(next: boolean) {
    if (disabled && next) return;
    if (next === open) return;
    open = next;
    onOpenChange?.(next);
    if (next) active = items.has(value) ? value : null;
    else if (trigger) renderer.focusElement?.(trigger.id);
  }
  function select(next: string) {
    if (disabled || items.get(next)?.disabled) return;
    if (value !== next) {
      value = next;
      onValueChange?.(next);
    }
    setOpen(false);
  }
  function setActive(next: string, scroll = false) {
    active = next;
    if (scroll && content)
      renderer.scrollToItem?.(content.id, [...items.keys()].indexOf(next));
  }
  function key(event: EventPayload) {
    if (disabled) return;
    const key = event.key?.toLowerCase(),
      enabled = [...items.values()].filter((item) => !item.disabled);
    if (key === "escape" || key === "tab") {
      setOpen(false);
      return;
    }
    if (key === "enter" || key === "space" || key === " ") {
      if (!open) setOpen(true);
      else if (active !== null) select(active);
      return;
    }
    if (
      ["up", "arrowup", "down", "arrowdown", "home", "end"].includes(key ?? "")
    ) {
      if (!enabled.length) return;
      setOpen(true);
      let index = enabled.findIndex((item) => item.value === active);
      if (index < 0 && (key === "up" || key === "arrowup")) index = 0;
      if (key === "home") index = 0;
      else if (key === "end") index = enabled.length - 1;
      else
        index =
          (index +
            (key === "up" || key === "arrowup" ? -1 : 1) +
            enabled.length) %
          enabled.length;
      setActive(enabled[index]!.value, true);
      return;
    }
    const char = event.keyChar ?? event.key;
    if (
      !char ||
      [...char].length !== 1 ||
      event.modifiers?.ctrl ||
      event.modifiers?.alt ||
      event.modifiers?.cmd
    )
      return;
    const now = performance.now();
    query = now - at > 700 ? char : query + char;
    at = now;
    const found = enabled.find((item) =>
      item.label.toLowerCase().startsWith(query.toLowerCase()),
    );
    if (found) {
      setActive(found.value, open);
      if (!open) select(found.value);
    }
  }
  const context: SelectionContext = {
    get value() {
      return value;
    },
    get open() {
      return open;
    },
    get disabled() {
      return disabled;
    },
    get inputValue() {
      return "";
    },
    get active() {
      return active;
    },
    get items() {
      return items;
    },
    get visible() {
      return [...items.values()];
    },
    get trigger() {
      return trigger;
    },
    setTrigger(next) {
      trigger = next;
    },
    setContent(next) {
      content = next;
    },
    setOpen,
    setInput() {},
    setActive,
    select,
    key,
    register(item) {
      items.set(item.value, item);
      return () => {
        if (items.get(item.value) === item) items.delete(item.value);
      };
    },
    selected(next) {
      return value === next;
    },
  };
  setContext(SELECT, context);
</script>

<View
  {...props}
  style={{
    display: "flex",
    position: "relative",
    alignItems: "start",
    ...props.style,
  }}>{@render children?.()}</View
>
