import {
  KIT_COMPONENTS,
  KIT_MODEL_PROPS,
  kitHostProps,
  normalizeKitProps,
  type KitComponentProps,
  type KitModelValue,
  type KitModelName,
} from "@gpui-native/runtime/kit"
import { h, type FunctionalComponent } from "@vue/runtime-core"
export type * from "@gpui-native/runtime/kit"

type VueKitProps<K extends keyof KitComponentProps> = KitComponentProps[K] & {
  [P in KitModelName<K> as `onUpdate:${P}`]?: (value: KitModelValue<K>) => void
} & {
  modelValue?: KitModelValue<K>
  "onUpdate:modelValue"?: (value: KitModelValue<K>) => void
}
function component<K extends keyof KitComponentProps>(
  name: K,
): FunctionalComponent<VueKitProps<K>> {
  const view: FunctionalComponent<VueKitProps<K>> = (props, { slots }) => {
    const normalized = normalizeKitProps(props as Record<string, unknown>)
    const { modelValue, "onUpdate:modelValue": update, ...host } = normalized
    const model = KIT_MODEL_PROPS[name as keyof typeof KIT_MODEL_PROPS]
    if (model) {
      if ("modelValue" in normalized) host[model] = modelValue
      const namedUpdate = host[`onUpdate:${model}`]
      delete host[`onUpdate:${model}`]
      const changed = host.onValueChange
      host.onValueChange = (value: unknown) => {
        if (typeof update === "function") update(value)
        if (typeof namedUpdate === "function") namedUpdate(value)
        if (typeof changed === "function") changed(value)
      }
    }
    return h(KIT_COMPONENTS[name], kitHostProps(host), slots.default?.())
  }
  view.inheritAttrs = false
  view.displayName = `Kit${name}`
  return view
}

export const Button = component("Button")
export const Checkbox = component("Checkbox")
export const Switch = component("Switch")
export const Radio = component("Radio")
export const Toggle = component("Toggle")
export const Badge = component("Badge")
export const Tag = component("Tag")
export const Spinner = component("Spinner")
export const Skeleton = component("Skeleton")
export const Separator = component("Separator")
export const Progress = component("Progress")
export const ProgressCircle = component("ProgressCircle")
export const Rating = component("Rating")
export const Pagination = component("Pagination")
export const Link = component("Link")
export const Avatar = component("Avatar")
export const GroupBox = component("GroupBox")
export const Collapsible = component("Collapsible")
export const Slider = component("Slider")
export const Calendar = component("Calendar")
export const DatePicker = component("DatePicker")
export const ColorPicker = component("ColorPicker")
export const Tabs = component("Tabs")
export const RadioGroup = component("RadioGroup")
export const Accordion = component("Accordion")
export const DescriptionList = component("DescriptionList")
export const Breadcrumb = component("Breadcrumb")
export const Alert = component("Alert")
export const Empty = component("Empty")
export const Icon = component("Icon")
export const Kbd = component("Kbd")
export const DataTable = component("DataTable")
export const Popover = component("Popover")
export const Dialog = component("Dialog")
export const Sheet = component("Sheet")
export const Select = component("Select")
export const Combobox = component("Combobox")
export const Form = component("Form")
export const Field = component("Field")
export const Stepper = component("Stepper")
export const Sidebar = component("Sidebar")
export const SidebarHeader = component("SidebarHeader")
export const SidebarFooter = component("SidebarFooter")
export const SidebarToggle = component("SidebarToggle")
export const StatusBar = component("StatusBar")
export const Clipboard = component("Clipboard")
export const Message = component("Message")
export const MessageGroup = component("MessageGroup")
export const MessageHeader = component("MessageHeader")
export const MessageContent = component("MessageContent")
export const MessageFooter = component("MessageFooter")
export const Bubble = component("Bubble")
export const BubbleGroup = component("BubbleGroup")
export const Marker = component("Marker")
export const AttachmentGroup = component("AttachmentGroup")
export const Input = component("Input")
export const Textarea = component("Textarea")
export const Editor = component("Editor")
export const NumberInput = component("NumberInput")
export const OtpInput = component("OtpInput")
export const Tree = component("Tree")
export const Carousel = component("Carousel")
export const DropdownMenu = component("DropdownMenu")
export const ContextMenu = component("ContextMenu")
export const Theme = component("Theme")

export const LineChart = component("LineChart")
export const AreaChart = component("AreaChart")
export const BarChart = component("BarChart")
export const PieChart = component("PieChart")
export const CandlestickChart = component("CandlestickChart")
export const RadarChart = component("RadarChart")
export const SankeyChart = component("SankeyChart")
export const Dock = component("Dock")
export const Resizable = component("Resizable")
export const Command = component("Command")
export const List = component("List")

export const Label = component("Label")
export const ShimmerText = component("ShimmerText")
export const Tooltip = component("Tooltip")
export const HoverCard = component("HoverCard")
export const Notification = component("Notification")
export const Attachment = component("Attachment")

export const Settings = component("Settings")
export const MessageScroller = component("MessageScroller")
export const TitleBar = component("TitleBar")
export const WindowBorder = component("WindowBorder")
