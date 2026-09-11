<script lang="ts">
  import { View } from "../components.js";
  import { setContext } from "../public.js";
  import { SvelteMap } from "../reactivity.js";
  import { useGpuiRequired } from "../context.svelte.js";
  import { COMBOBOX } from "../control-context.svelte.js";
  import type {
    ComboboxProps,
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
    multiple = false,
    defaultInputValue = "",
    inputValue = $bindable(defaultInputValue),
    onValueChange,
    onOpenChange,
    onInputValueChange,
    filter,
    items: suppliedItems,
    itemToStringValue,
    autoHighlight = true,
    children,
    ...props
  }: ComboboxProps = $props();
  let active = $state<string | null>(null),
    trigger = $state.raw<ElementRef | null>(null),
    content = $state.raw<ElementRef | null>(null);
  const items = new SvelteMap<string, SelectionItem>(),
    renderer = useGpuiRequired();
  // Supplied data owns the list: reading its mounted-item registry here would
  // make filtered snippet mounting feed back into its own source collection.
  const all = $derived(
    suppliedItems
      ? suppliedItems.map((value) => ({
          value,
          label: itemToStringValue?.(value) ?? value,
          disabled: false,
          ref: null,
        }))
      : [...items.values()],
  );
  const label = (value: string) =>
    itemToStringValue?.(value) ??
    (suppliedItems ? value : (items.get(value)?.label ?? value));
  const visible = $derived.by(() => {
    const query = inputValue.trim().toLocaleLowerCase();
    if (filter === null) return all;
    if (filter)
      return all.filter((item) => filter(item.value, inputValue, label));
    return all
      .filter((item) => label(item.value).toLocaleLowerCase().includes(query))
      .sort(
        (a, b) =>
          Number(label(b.value).toLocaleLowerCase().startsWith(query)) -
          Number(label(a.value).toLocaleLowerCase().startsWith(query)),
      );
  });
  function setOpen(next: boolean) {
    if (disabled && next) return;
    if (next === open) return;
    open = next;
    onOpenChange?.(next);
    if (!next && trigger) renderer.focusElement?.(trigger.id);
  }
  function selected(next: string) {
    return Array.isArray(value) ? value.includes(next) : value === next;
  }
  function setInput(next: string) {
    if (disabled) return;
    inputValue = next;
    onInputValueChange?.(next);
    setOpen(true);
  }
  function select(next: string) {
    if (disabled || items.get(next)?.disabled) return;
    value = multiple
      ? selected(next)
        ? (Array.isArray(value) ? value : []).filter((v) => v !== next)
        : [...(Array.isArray(value) ? value : []), next]
      : next;
    onValueChange?.(value);
    if (!multiple) {
      inputValue = label(next);
      onInputValueChange?.(inputValue);
      setOpen(false);
    } else {
      inputValue = "";
      onInputValueChange?.(inputValue);
    }
  }
  $effect(() => {
    if (
      autoHighlight &&
      !visible.some(
        (item) =>
          item.value === active &&
          !item.disabled &&
          !items.get(item.value)?.disabled,
      )
    )
      active =
        visible.find(
          (item) => !item.disabled && !items.get(item.value)?.disabled,
        )?.value ?? null;
  });
  function key(event: EventPayload) {
    if (disabled) return;
    const key = event.key?.toLowerCase(),
      enabled = visible.filter(
        (item) => !item.disabled && !items.get(item.value)?.disabled,
      );
    if (key === "escape" || key === "tab") {
      setOpen(false);
      return;
    }
    if (key === "enter") {
      if (!open) setOpen(true);
      else if (active !== null) select(active);
      return;
    }
    if (
      !["up", "arrowup", "down", "arrowdown", "home", "end"].includes(
        key ?? "",
      ) ||
      !enabled.length
    )
      return;
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
    active = enabled[index]!.value;
    if (content)
      renderer.scrollToItem?.(
        content.id,
        visible.findIndex((item) => item.value === active),
      );
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
      return inputValue;
    },
    get active() {
      return active;
    },
    get items() {
      return items;
    },
    get visible() {
      return visible;
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
    setInput,
    setActive(next) {
      active = next;
    },
    select,
    key,
    register(item) {
      items.set(item.value, item);
      return () => {
        if (items.get(item.value) === item) items.delete(item.value);
      };
    },
    selected,
  };
  setContext(COMBOBOX, context);
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
