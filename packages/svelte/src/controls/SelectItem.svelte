<script lang="ts">
  import { View } from "../components.js";
  import { selection, SELECT } from "../control-context.svelte.js";
  import type { SelectItemProps } from "../control-types.js";
  import type { ElementRef } from "../types.js";
  let {
    value,
    textValue,
    disabled = false,
    style,
    content,
    children,
    ...props
  }: SelectItemProps = $props();
  const context = selection(SELECT);
  let ref = $state.raw<ElementRef | null>(null);
  $effect(() =>
    context.register({ value, label: textValue ?? value, disabled, ref }),
  );
  const visible = $derived(true);
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
