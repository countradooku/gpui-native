<script lang="ts">
  import { View } from "../components.js";
  import { selection, COMBOBOX } from "../control-context.svelte.js";
  import type { ComboboxItemProps } from "../control-types.js";
  import type { ElementRef } from "../types.js";
  let {
    value,
    textValue,
    disabled = false,
    style,
    content,
    children,
    ...props
  }: ComboboxItemProps = $props();
  const context = selection(COMBOBOX);
  let ref = $state.raw<ElementRef | null>(null);
  $effect(() =>
    context.register({ value, label: textValue ?? value, disabled, ref }),
  );
  const visible = $derived(
    context.visible.some((item) => item.value === value),
  );
  const itemState = $derived({
    selected: context.selected(value),
    highlighted: context.active === value,
    disabled: disabled || context.disabled,
  });
  const resolvedStyle = $derived(
    typeof style === "function" ? style(itemState) : style,
  );
</script>

<View
  {...props}
  bind:ref
  role="option"
  aria-selected={context.selected(value)}
  tabIndex={itemState.disabled || !context.open || !visible ? -1 : 0}
  style={{ ...resolvedStyle, ...(!visible ? { display: "none" } : {}) }}
  onClick={(event) => {
    if (!itemState.disabled) {
      props.onClick?.(event);
      context.select(value);
    }
  }}
  onMouseEnter={(event) => {
    props.onMouseEnter?.(event);
    if (!itemState.disabled) context.setActive(value);
  }}
  onKeyDown={(event) => {
    props.onKeyDown?.(event);
    context.key(event);
  }}
  >{#if content}{@render content(
      itemState,
    )}{:else}{@render children?.()}{/if}</View
>
