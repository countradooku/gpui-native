import type { EventPayload, HostProps } from "./types.js"

/** Shared retained events; specialized document events belong to the corresponding primitives. */
export type KitHostProps = Omit<
  HostProps,
  "onToggleFile" | "onShowMore" | "onLineClick" | "onLinkClick" | "onSubmit" | "onVisibleRange"
>
export type KitSize = "xsmall" | "small" | "medium" | "large"
export type KitVariant =
  | "default"
  | "primary"
  | "secondary"
  | "danger"
  | "success"
  | "warning"
  | "info"
  | "ghost"
  | "link"
export interface KitControlProps extends KitHostProps {
  disabled?: boolean
  size?: KitSize
}
export interface KitValueProps<T> {
  onValueChange?: (value: T) => void
}
export interface ButtonProps extends KitControlProps {
  label?: string
  variant?: KitVariant
  selected?: boolean
  loading?: boolean
  outline?: boolean
  compact?: boolean
  tooltip?: string
}
export interface CheckboxProps extends KitControlProps, KitValueProps<boolean> {
  label?: string
  checked?: boolean
}
export type SwitchProps = CheckboxProps
export type RadioProps = CheckboxProps
export type ToggleProps = CheckboxProps
export interface BadgeProps extends KitHostProps {
  count?: number
  max?: number
  dot?: boolean
  size?: KitSize
}
export interface TagProps extends KitHostProps {
  label?: string
  variant?: Exclude<KitVariant, "default" | "ghost" | "link">
  size?: KitSize
  outline?: boolean
}
export interface SpinnerProps extends KitHostProps {
  size?: KitSize
}
export interface SkeletonProps extends KitHostProps {
  secondary?: boolean
}
export interface SeparatorProps extends KitHostProps {
  vertical?: boolean
  dashed?: boolean
  label?: string
}
export interface ProgressProps extends KitHostProps {
  value?: number
}
export type ProgressCircleProps = ProgressProps
export interface RatingProps extends KitControlProps, KitValueProps<number> {
  value?: number
  max?: number
}
export interface PaginationProps extends KitControlProps, KitValueProps<number> {
  page?: number
  totalPages?: number
  visiblePages?: number
  compact?: boolean
}
export interface LinkProps extends KitHostProps {
  href?: string
  label?: string
  disabled?: boolean
}
export interface AvatarProps extends KitHostProps {
  src?: string
  name?: string
  size?: KitSize
}
export interface GroupBoxProps extends KitHostProps {
  title?: string
}
export interface CollapsibleProps extends KitHostProps {
  open?: boolean
}

export interface SliderProps extends KitHostProps, KitValueProps<number | [number, number]> {
  value?: number | [number, number]
  min?: number
  max?: number
  step?: number
  vertical?: boolean
  disabled?: boolean
}
export type KitDate = string | null | [string | null, string | null]
export interface CalendarProps extends KitHostProps, KitValueProps<KitDate> {
  value?: KitDate
  range?: boolean
  numberOfMonths?: number
  size?: KitSize
}
export interface DatePickerProps extends CalendarProps {
  placeholder?: string
  cleanable?: boolean
  disabled?: boolean
}
export interface ColorPickerProps extends KitHostProps, KitValueProps<string | null> {
  value?: string | null
  size?: KitSize
}
export type KitItem =
  | string
  | { label?: string; title?: string; content?: string; value?: string; disabled?: boolean }
export interface TabsProps extends KitHostProps, KitValueProps<number> {
  items?: KitItem[]
  selectedIndex?: number
  variant?: "pill" | "outline" | "segmented" | "underline"
  size?: KitSize
}
export interface RadioGroupProps extends KitControlProps, KitValueProps<number> {
  items?: KitItem[]
  selectedIndex?: number
}
export interface AccordionProps extends KitControlProps, KitValueProps<number[]> {
  items?: KitItem[]
  openIndices?: number[]
  multiple?: boolean
  bordered?: boolean
}
export interface DescriptionListProps extends KitHostProps {
  items?: KitItem[]
  columns?: number
  vertical?: boolean
  bordered?: boolean
  size?: KitSize
}
export interface BreadcrumbProps extends KitHostProps, KitValueProps<number> {
  items?: KitItem[]
}
export interface AlertProps extends KitHostProps {
  title?: string
  message?: string
  variant?: "default" | "info" | "success" | "warning" | "error"
}
export interface EmptyProps extends KitHostProps {
  title?: string
  description?: string
}
export interface IconProps extends KitHostProps {
  name: string
  size?: KitSize
}
export interface KbdProps extends KitHostProps {
  keystroke: string
}
export interface KitTableColumn {
  key: string
  label?: string
  width?: number
  sortable?: boolean
  resizable?: boolean
  fixed?: boolean
}
export type KitTableEvent =
  | { type: "sort"; column: number; direction: "none" | "ascending" | "descending" }
  | { type: "selectRow" | "doubleClickRow" | "contextRow"; row: number }
  | { type: "selectColumn"; column: number }
  | { type: "selectCell" | "doubleClickCell" | "contextCell"; row: number; column: number }
  | { type: "columnWidths"; widths: number[] }
  | { type: "moveColumn"; from: number; to: number }
  | { type: "clearSelection" }
export interface DataTableProps extends KitHostProps, KitValueProps<KitTableEvent> {
  onVisibleRange?: HostProps["onVisibleRange"]
  columns?: KitTableColumn[]
  rows?: Record<string, unknown>[]
  stripe?: boolean
  bordered?: boolean
  loading?: boolean
  size?: KitSize
}
export interface PopoverProps extends KitControlProps, KitValueProps<boolean> {
  label?: string
  open?: boolean
  defaultOpen?: boolean
  overlayClosable?: boolean
  appearance?: boolean
  anchor?:
    | "top-left"
    | "top-center"
    | "top-right"
    | "bottom-left"
    | "bottom-center"
    | "bottom-right"
}
export interface DialogProps extends KitHostProps, KitValueProps<boolean> {
  open?: boolean
  title?: string
  dialogWidth?: number
  overlay?: boolean
  overlayClosable?: boolean
  closeButton?: boolean
  keyboard?: boolean
}
export interface SheetProps extends KitHostProps, KitValueProps<boolean> {
  open?: boolean
  title?: string
  placement?: "left" | "right" | "top" | "bottom"
  sheetSize?: number
  overlay?: boolean
  overlayClosable?: boolean
  resizable?: boolean
}
export interface SelectProps extends KitControlProps, KitValueProps<string | null> {
  items?: KitItem[]
  value?: string | null
  searchable?: boolean
  placeholder?: string
  searchPlaceholder?: string
  cleanable?: boolean
  appearance?: boolean
}
export interface ComboboxProps extends KitControlProps, KitValueProps<string | string[] | null> {
  items?: KitItem[]
  value?: string | string[] | null
  multiple?: boolean
  searchable?: boolean
  placeholder?: string
  searchPlaceholder?: string
  cleanable?: boolean
  appearance?: boolean
}

export interface FormItem {
  label?: string
  description?: string
  required?: boolean
}
export interface FormProps extends KitHostProps {
  items?: FormItem[]
  columns?: number
  vertical?: boolean
  size?: KitSize
}
export interface FieldProps extends KitHostProps {
  label?: string
  description?: string
  required?: boolean
}
export interface StepperProps extends KitControlProps, KitValueProps<number> {
  items?: KitItem[]
  selectedIndex?: number
  vertical?: boolean
  textCenter?: boolean
}
export interface SidebarProps extends KitHostProps, KitValueProps<number> {
  items?: KitItem[]
  selectedIndex?: number
  collapsed?: boolean
  side?: "left" | "right"
}
export type SidebarHeaderProps = KitHostProps
export type SidebarFooterProps = KitHostProps
export interface SidebarToggleProps extends KitHostProps, KitValueProps<boolean> {
  collapsed?: boolean
  side?: "left" | "right"
}
export type StatusBarProps = KitHostProps
export interface ClipboardProps extends KitHostProps, KitValueProps<boolean> {
  value?: string
  tooltip?: string
}
export interface MessageProps extends KitHostProps {
  alignment?: "start" | "end"
}
export type MessageGroupProps = KitHostProps
export type MessageHeaderProps = KitHostProps
export type MessageContentProps = KitHostProps
export type MessageFooterProps = KitHostProps
export interface BubbleProps extends MessageProps {
  variant?: "default" | "outline" | "ghost"
}
export type BubbleGroupProps = KitHostProps
export interface MarkerProps extends KitHostProps {
  variant?: "default" | "separator"
  loading?: boolean
}
export type AttachmentGroupProps = KitHostProps
export interface InputProps extends KitControlProps, KitValueProps<string> {
  onSubmit?: HostProps["onSubmit"]
  value?: string
  placeholder?: string
  readonly?: boolean
  masked?: boolean
  cleanable?: boolean
  appearance?: boolean
  bordered?: boolean
}
export interface TextareaProps extends KitHostProps, KitValueProps<string> {
  onSubmit?: HostProps["onSubmit"]
  value?: string
  placeholder?: string
  disabled?: boolean
  readonly?: boolean
  rows?: number
  softWrap?: boolean
  appearance?: boolean
  bordered?: boolean
}
export interface EditorProps extends KitHostProps, KitValueProps<string> {
  onSubmit?: HostProps["onSubmit"]
  value?: string
  placeholder?: string
  disabled?: boolean
  readonly?: boolean
  language?: string
  lineNumbers?: boolean
  softWrap?: boolean
  searchable?: boolean
  appearance?: boolean
  bordered?: boolean
}
/** NumberInput keeps a string value so incomplete edits such as '-' remain representable. */
export interface NumberInputProps extends KitControlProps, KitValueProps<string> {
  onSubmit?: HostProps["onSubmit"]
  value?: string
  placeholder?: string
  min?: number
  max?: number
  step?: number
  appearance?: boolean
}
export interface OtpInputProps extends KitControlProps, KitValueProps<string> {
  onSubmit?: HostProps["onSubmit"]
  value?: string
  length?: number
  groups?: number
  masked?: boolean
}
export interface KitTreeItem {
  id: string
  label?: string
  children?: KitTreeItem[]
  expanded?: boolean
  disabled?: boolean
}
export type KitTreeEvent =
  | { type: "selected"; id: string | null }
  | { type: "expanded" | "collapsed"; id: string }
export interface TreeProps extends KitHostProps, KitValueProps<KitTreeEvent> {
  items?: KitTreeItem[]
  selectedId?: string
}
export interface CarouselProps extends KitHostProps, KitValueProps<number> {
  selectedIndex?: number
  vertical?: boolean
  looping?: boolean
  controls?: boolean
  pagination?: boolean
}
export interface KitMenuItem {
  label?: string
  disabled?: boolean
  checked?: boolean
  separator?: boolean
}
export interface DropdownMenuProps extends KitControlProps, KitValueProps<number> {
  items?: (string | KitMenuItem)[]
  label?: string
}
export interface ContextMenuProps extends KitHostProps, KitValueProps<number> {
  items?: (string | KitMenuItem)[]
}
export interface KitSemanticTokens {
  colors?: Partial<
    Record<
      | "background"
      | "foreground"
      | "surface"
      | "surface_foreground"
      | "primary"
      | "primary_foreground"
      | "secondary"
      | "secondary_foreground"
      | "muted"
      | "muted_foreground"
      | "accent"
      | "accent_foreground"
      | "destructive"
      | "destructive_foreground"
      | "border"
      | "input"
      | "ring",
      string
    >
  >
  radius?: Partial<Record<"none" | "sm" | "md" | "lg" | "xl" | "full", number>>
  spacing?: Partial<Record<"xxs" | "xs" | "sm" | "md" | "lg" | "xl" | "xxl", number>>
  typography?: { sans?: string; mono?: string } & Partial<
    Record<
      "xs" | "sm" | "md" | "lg" | "xl" | "mono_md",
      { size?: number; line_height?: number; weight?: number }
    >
  >
}
/** Kit's theme and locale are application-wide, matching the upstream library. */
export interface ThemeProps extends KitHostProps {
  mode?: "system" | "light" | "dark"
  locale?: string
  tokens?: KitSemanticTokens
  fontFamily?: string
  fontSize?: number
  monoFontFamily?: string
  monoFontSize?: number
  radius?: number
  shadow?: boolean
  focusRing?: boolean
}

export interface ChartDatum {
  label: string
  value?: number
  open?: number
  high?: number
  low?: number
  close?: number
  color?: string
}
export interface ChartProps extends KitHostProps {
  data?: ChartDatum[]
  name?: string
  grid?: boolean
}
export interface LineChartProps extends ChartProps {
  stroke?: string
  curve?: "linear" | "natural" | "step"
  dot?: boolean
  xAxis?: boolean
  tickMargin?: number
}
export interface AreaChartProps extends LineChartProps {
  fill?: string
}
export interface BarChartProps extends ChartProps {
  fill?: string
  horizontal?: boolean
  labelAxis?: boolean
  valueAxis?: boolean
  tickMargin?: number
}
export interface PieChartProps extends KitHostProps {
  data?: ChartDatum[]
  innerRadius?: number
  outerRadius?: number
  padAngle?: number
}
export interface CandlestickChartProps extends ChartProps {
  xAxis?: boolean
  tickMargin?: number
  bodyWidthRatio?: number
}
export interface RadarChartProps extends ChartProps {
  stroke?: string
  fill?: string
  gridLevels?: number
  maxValue?: number
  dot?: boolean
}
export interface SankeyChartProps extends KitHostProps {
  data?: ChartDatum[]
  links?: { source: number; target: number; value: number }[]
  nodeWidth?: number
  nodePadding?: number
  linkOpacity?: number
}
export interface DockPanel {
  id: string
  title?: string
  closable?: boolean
  placement?: "center" | "left" | "right" | "bottom"
}
/** Serializable Kit dock state, intended to be stored and passed back unchanged. */
export type DockLayout = Record<string, unknown>
export interface DockProps extends KitHostProps, KitValueProps<DockLayout> {
  panels?: DockPanel[]
  layout?: DockLayout
  locked?: boolean
}
export interface ResizableProps extends KitHostProps, KitValueProps<number[]> {
  sizes?: number[]
  minSize?: number
  maxSize?: number
  vertical?: boolean
}
export type KitListEvent = { type: "select" | "confirm"; index: number | null } | { type: "cancel" }
export type KitCommandEvent = KitListEvent | { type: "query"; value: string }
export interface CommandProps extends KitHostProps, KitValueProps<KitCommandEvent> {
  items?: (string | KitMenuItem)[]
  query?: string
  searchable?: boolean
  filterable?: boolean
  placeholder?: string
  loading?: boolean
  bordered?: boolean
}
export interface ListProps extends KitHostProps, KitValueProps<KitListEvent> {
  items?: string[]
  query?: string
  selectedIndex?: number
  searchable?: boolean
  placeholder?: string
  size?: KitSize
}

export interface LabelProps extends KitHostProps {
  label?: string
  masked?: boolean
}
export interface ShimmerTextProps extends KitHostProps {
  label?: string
  duration?: number
  reverse?: boolean
  once?: boolean
}
export interface TooltipProps extends KitHostProps {
  label?: string
}
export interface HoverCardProps extends KitHostProps, KitValueProps<boolean> {
  label?: string
  openDelay?: number
  closeDelay?: number
  appearance?: boolean
}
export interface NotificationProps extends KitHostProps, KitValueProps<boolean> {
  open?: boolean
  title?: string
  message?: string
  variant?: "info" | "success" | "warning" | "error"
  autohide?: boolean
}
export interface AttachmentProps extends KitHostProps, KitValueProps<boolean> {
  title?: string
  description?: string
  src?: string
  status?: "pending" | "uploading" | "processing" | "failed" | "complete"
  vertical?: boolean
  size?: KitSize
}

export interface SettingItem {
  title?: string
  description?: string
}
export interface SettingGroup {
  title?: string
  description?: string
  items: SettingItem[]
}
export interface SettingPage {
  title?: string
  description?: string
  groups: SettingGroup[]
}
/** Children map to settings items in page/group/item order. Each child owns its value and events. */
export interface SettingsProps extends KitHostProps {
  pages?: SettingPage[]
  defaultSelectedIndex?: number
  sidebarWidth?: number
  size?: KitSize
}
/** Decrease firstIndex when prepending history to preserve the reader's scroll anchor. */
export interface MessageScrollerProps extends KitHostProps {
  firstIndex?: number
  scrollbar?: boolean
  jumpButton?: boolean
}
export type TitleBarProps = KitHostProps
export interface WindowBorderProps extends KitHostProps {
  shadowSize?: number
  resizeHitSize?: number
}

export interface KitComponentProps {
  Settings: SettingsProps
  MessageScroller: MessageScrollerProps
  TitleBar: TitleBarProps
  WindowBorder: WindowBorderProps

  Label: LabelProps
  ShimmerText: ShimmerTextProps
  Tooltip: TooltipProps
  HoverCard: HoverCardProps
  Notification: NotificationProps
  Attachment: AttachmentProps

  LineChart: LineChartProps
  AreaChart: AreaChartProps
  BarChart: BarChartProps
  PieChart: PieChartProps
  CandlestickChart: CandlestickChartProps
  RadarChart: RadarChartProps
  SankeyChart: SankeyChartProps
  Dock: DockProps
  Resizable: ResizableProps
  Command: CommandProps
  List: ListProps

  Button: ButtonProps
  Checkbox: CheckboxProps
  Switch: SwitchProps
  Radio: RadioProps
  Toggle: ToggleProps
  Badge: BadgeProps
  Tag: TagProps
  Spinner: SpinnerProps
  Skeleton: SkeletonProps
  Separator: SeparatorProps
  Progress: ProgressProps
  ProgressCircle: ProgressCircleProps
  Rating: RatingProps
  Pagination: PaginationProps
  Link: LinkProps
  Avatar: AvatarProps
  GroupBox: GroupBoxProps
  Collapsible: CollapsibleProps
  Slider: SliderProps
  Calendar: CalendarProps
  DatePicker: DatePickerProps
  ColorPicker: ColorPickerProps
  Tabs: TabsProps
  RadioGroup: RadioGroupProps
  Accordion: AccordionProps
  DescriptionList: DescriptionListProps
  Breadcrumb: BreadcrumbProps
  Alert: AlertProps
  Empty: EmptyProps
  Icon: IconProps
  Kbd: KbdProps
  DataTable: DataTableProps
  Popover: PopoverProps
  Dialog: DialogProps
  Sheet: SheetProps
  Select: SelectProps
  Combobox: ComboboxProps
  Form: FormProps
  Field: FieldProps
  Stepper: StepperProps
  Sidebar: SidebarProps
  SidebarHeader: SidebarHeaderProps
  SidebarFooter: SidebarFooterProps
  SidebarToggle: SidebarToggleProps
  StatusBar: StatusBarProps
  Clipboard: ClipboardProps
  Message: MessageProps
  MessageGroup: MessageGroupProps
  MessageHeader: MessageHeaderProps
  MessageContent: MessageContentProps
  MessageFooter: MessageFooterProps
  Bubble: BubbleProps
  BubbleGroup: BubbleGroupProps
  Marker: MarkerProps
  AttachmentGroup: AttachmentGroupProps
  Input: InputProps
  Textarea: TextareaProps
  Editor: EditorProps
  NumberInput: NumberInputProps
  OtpInput: OtpInputProps
  Tree: TreeProps
  Carousel: CarouselProps
  DropdownMenu: DropdownMenuProps
  ContextMenu: ContextMenuProps
  Theme: ThemeProps
}

export const KIT_COMPONENTS = {
  Settings: "kit-settings",
  MessageScroller: "kit-message-scroller",
  TitleBar: "kit-title-bar",
  WindowBorder: "kit-window-border",

  Label: "kit-label",
  ShimmerText: "kit-shimmer-text",
  Tooltip: "kit-tooltip",
  HoverCard: "kit-hover-card",
  Notification: "kit-notification",
  Attachment: "kit-attachment",

  LineChart: "kit-line-chart",
  AreaChart: "kit-area-chart",
  BarChart: "kit-bar-chart",
  PieChart: "kit-pie-chart",
  CandlestickChart: "kit-candlestick-chart",
  RadarChart: "kit-radar-chart",
  SankeyChart: "kit-sankey-chart",
  Dock: "kit-dock",
  Resizable: "kit-resizable",
  Command: "kit-command",
  List: "kit-list",

  Button: "kit-button",
  Checkbox: "kit-checkbox",
  Switch: "kit-switch",
  Radio: "kit-radio",
  Toggle: "kit-toggle",
  Badge: "kit-badge",
  Tag: "kit-tag",
  Spinner: "kit-spinner",
  Skeleton: "kit-skeleton",
  Separator: "kit-separator",
  Progress: "kit-progress",
  ProgressCircle: "kit-progress-circle",
  Rating: "kit-rating",
  Pagination: "kit-pagination",
  Link: "kit-link",
  Avatar: "kit-avatar",
  GroupBox: "kit-group-box",
  Collapsible: "kit-collapsible",
  Slider: "kit-slider",
  Calendar: "kit-calendar",
  DatePicker: "kit-date-picker",
  ColorPicker: "kit-color-picker",
  Tabs: "kit-tabs",
  RadioGroup: "kit-radio-group",
  Accordion: "kit-accordion",
  DescriptionList: "kit-description-list",
  Breadcrumb: "kit-breadcrumb",
  Alert: "kit-alert",
  Empty: "kit-empty",
  Icon: "kit-icon",
  Kbd: "kit-kbd",
  DataTable: "kit-data-table",
  Popover: "kit-popover",
  Dialog: "kit-dialog",
  Sheet: "kit-sheet",
  Select: "kit-select",
  Combobox: "kit-combobox",
  Form: "kit-form",
  Field: "kit-field",
  Stepper: "kit-stepper",
  Sidebar: "kit-sidebar",
  SidebarHeader: "kit-sidebar-header",
  SidebarFooter: "kit-sidebar-footer",
  SidebarToggle: "kit-sidebar-toggle",
  StatusBar: "kit-status-bar",
  Clipboard: "kit-clipboard",
  Message: "kit-message",
  MessageGroup: "kit-message-group",
  MessageHeader: "kit-message-header",
  MessageContent: "kit-message-content",
  MessageFooter: "kit-message-footer",
  Bubble: "kit-bubble",
  BubbleGroup: "kit-bubble-group",
  Marker: "kit-marker",
  AttachmentGroup: "kit-attachment-group",
  Input: "kit-input",
  Textarea: "kit-textarea",
  Editor: "kit-editor",
  NumberInput: "kit-number-input",
  OtpInput: "kit-otp-input",
  Tree: "kit-tree",
  Carousel: "kit-carousel",
  DropdownMenu: "kit-dropdown-menu",
  ContextMenu: "kit-context-menu",
  Theme: "kit-theme",
} as const satisfies Record<keyof KitComponentProps, string>

/** Lower typed callbacks before the retained mutation protocol serializes props. */
export function kitHostProps(props: Record<string, unknown>): Record<string, unknown> {
  const { onValueChange, ...host } = normalizeKitProps(props)
  if (typeof onValueChange === "function") {
    const change = host.onChange as ((event: EventPayload) => void) | undefined
    host.onChange = (event: EventPayload) => {
      const value: unknown = JSON.parse(event.value ?? "null")
      onValueChange(value)
      change?.(event)
    }
  }
  return host
}

/** The property controlled by a framework's default two-way binding. */
export const KIT_MODEL_PROPS = {
  Dock: "layout",
  Resizable: "sizes",
  Notification: "open",
  Checkbox: "checked",
  Switch: "checked",
  Radio: "checked",
  Toggle: "checked",
  Slider: "value",
  Calendar: "value",
  DatePicker: "value",
  ColorPicker: "value",
  Rating: "value",
  Pagination: "page",
  Tabs: "selectedIndex",
  RadioGroup: "selectedIndex",
  Accordion: "openIndices",
  Popover: "open",
  Dialog: "open",
  Sheet: "open",
  Select: "value",
  Combobox: "value",
  Stepper: "selectedIndex",
  Sidebar: "selectedIndex",
  SidebarToggle: "collapsed",
  Input: "value",
  Textarea: "value",
  Editor: "value",
  NumberInput: "value",
  OtpInput: "value",
  Carousel: "selectedIndex",
} as const satisfies Partial<Record<keyof KitComponentProps, string>>
export type KitModelName<K extends keyof KitComponentProps> = K extends keyof typeof KIT_MODEL_PROPS
  ? (typeof KIT_MODEL_PROPS)[K]
  : never
export type KitModelValue<K extends keyof KitComponentProps> =
  KitModelName<K> extends keyof KitComponentProps[K]
    ? Exclude<KitComponentProps[K][KitModelName<K>], undefined>
    : never

export type KitIntrinsicElements = {
  [K in keyof KitComponentProps as (typeof KIT_COMPONENTS)[K]]: KitComponentProps[K]
}

const booleanKitProps = new Set([
  "scrollbar",
  "jumpButton",
  "autohide",
  "reverse",
  "once",
  "filterable",
  "locked",
  "grid",
  "xAxis",
  "horizontal",
  "labelAxis",
  "valueAxis",
  "shadow",
  "focusRing",
  "looping",
  "controls",
  "pagination",
  "readonly",
  "masked",
  "lineNumbers",
  "softWrap",
  "required",
  "collapsed",
  "textCenter",
  "searchable",
  "defaultOpen",
  "overlayClosable",
  "appearance",
  "overlay",
  "closeButton",
  "keyboard",
  "resizable",
  "stripe",
  "checked",
  "disabled",
  "selected",
  "loading",
  "outline",
  "compact",
  "dot",
  "secondary",
  "vertical",
  "dashed",
  "open",
  "range",
  "cleanable",
  "bordered",
  "multiple",
])
/** Native boolean attributes support presence syntax without coercing string-valued props. */
export function normalizeKitProp(key: string, value: unknown): unknown {
  return value === "" && booleanKitProps.has(key) ? true : value
}

export function normalizeKitPropName(key: string): string {
  if (key.startsWith("aria-") || key.startsWith("data-")) return key
  return key.replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase())
}
export function normalizeKitProps(props: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(props).map(([key, value]) => [normalizeKitPropName(key), value]),
  )
}
