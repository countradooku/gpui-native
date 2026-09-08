import type {
  AudioBufferState as GeneratedAudioBufferState,
  DebugFrameOverlayStats as GeneratedDebugFrameOverlayStats,
  EdgeInsets as GeneratedEdgeInsets,
  EventModifiers as GeneratedEventModifiers,
  EventPayload as GeneratedEventPayload,
  HighlightMatch as GeneratedHighlightMatch,
  TimelineState as GeneratedTimelineState,
  WindowInsets as GeneratedWindowInsets,
  WindowOptions as GeneratedWindowOptions,
  WindowSize as GeneratedWindowSize,
} from "@gpui-native/core"
import type { VNodeRef } from "@vue/runtime-core"

/** Native ABI types are generated from the Rust N-API surface. */
export type EventModifiers = GeneratedEventModifiers
export type EventPayload = GeneratedEventPayload
export type WindowSize = GeneratedWindowSize
export type TimelineState = GeneratedTimelineState
export type AudioBufferState = GeneratedAudioBufferState

export type NativeWindowOptions = Omit<GeneratedWindowOptions, "windowBackground"> & {
  windowBackground?: "opaque" | "transparent" | "blurred"
  /** Open without taking keyboard focus. Ignored by GPUI on Linux. */
  focus?: boolean
  /** Open hidden; call `activateWindow()`/`useGpuiWindow().activate()` to reveal it. */
  show?: boolean
}

export type DimensionValue = number | string

/** CSS cursor keywords implemented by GPUI. */
export type CursorValue =
  | "default"
  | "auto"
  | "pointer"
  | "text"
  | "vertical-text"
  | "crosshair"
  | "grab"
  | "grabbing"
  | "move"
  | "all-scroll"
  | "col-resize"
  | "row-resize"
  | "ew-resize"
  | "ns-resize"
  | "nwse-resize"
  | "nesw-resize"
  | "n-resize"
  | "e-resize"
  | "s-resize"
  | "w-resize"
  | "ne-resize"
  | "nw-resize"
  | "se-resize"
  | "sw-resize"
  | "not-allowed"
  | "no-drop"
  | "alias"
  | "copy"
  | "context-menu"

export interface BoxShadow {
  offsetX: number
  offsetY: number
  blurRadius: number
  spreadRadius: number
  color: string
}

export interface MotionStyle {
  width?: number
  height?: number
  opacity?: number
  top?: number
  right?: number
  bottom?: number
  left?: number
  borderRadius?: number
}

export type MotionEase =
  | "linear"
  | "ease"
  | "easeIn"
  | "easeOut"
  | "easeInOut"
  | [number, number, number, number]

export interface MotionTransition {
  /** Native interpolation model. */
  type?: "tween" | "spring"
  /** Duration in seconds. */
  duration?: number
  /** Delay in seconds. */
  delay?: number
  ease?: MotionEase
  /** Spring stiffness (k). */
  stiffness?: number
  /** Spring damping (c). */
  damping?: number
  /** Spring mass (m). */
  mass?: number
  /** Initial normalized spring velocity. */
  velocity?: number
  /** Additional playback cycles after the first. */
  repeat?: number
  repeatType?: "loop" | "reverse"
  /** Delay per item for native staggered entrances. */
  stagger?: number
  /** Zero-based index multiplied by `stagger`. */
  staggerIndex?: number
}

export interface MotionKeyframe {
  /** Normalized timeline position. Omit for even spacing. */
  at?: number
  value: MotionStyle
  ease?: MotionEase
}

export interface MotionProps {
  initial?: MotionStyle | false
  animate: MotionStyle | readonly MotionKeyframe[]
  transition?: MotionTransition
}

/** CSS-like properties implemented by gpui-vue's native style mapper. */
export interface StyleDesc {
  display?: string
  visibility?: string
  flexDirection?: string
  flexWrap?: string
  flexGrow?: number
  flexShrink?: number
  flexBasis?: number
  alignItems?: string
  alignSelf?: string
  alignContent?: string
  justifyContent?: string
  gap?: number
  rowGap?: number
  columnGap?: number
  gridTemplateColumns?: number
  gridTemplateRows?: number
  gridColumnMin?: "zero" | "min-content" | "max-content"
  gridRowMin?: "zero" | "min-content" | "max-content"

  width?: DimensionValue
  height?: DimensionValue
  minWidth?: DimensionValue
  minHeight?: DimensionValue
  maxWidth?: DimensionValue
  maxHeight?: DimensionValue

  padding?: number
  paddingTop?: number
  paddingRight?: number
  paddingBottom?: number
  paddingLeft?: number
  margin?: number
  marginTop?: number
  marginRight?: number
  marginBottom?: number
  marginLeft?: number

  position?: string
  top?: number
  right?: number
  bottom?: number
  left?: number

  background?:
    | string
    | {
        type: "linear-gradient"
        angle: number
        stops: [{ color: string; position: number }, { color: string; position: number }]
        colorSpace?: "srgb" | "oklab"
      }
  backgroundColor?: string
  color?: string
  opacity?: number

  borderWidth?: number
  borderTopWidth?: number
  borderRightWidth?: number
  borderBottomWidth?: number
  borderLeftWidth?: number
  borderColor?: string
  borderRadius?: number
  borderTopLeftRadius?: number
  borderTopRightRadius?: number
  borderBottomLeftRadius?: number
  borderBottomRightRadius?: number
  boxShadow?: BoxShadow

  fontSize?: number
  fontFamily?: string
  fontWeight?: string | number
  textAlign?: string
  lineHeight?: number
  whiteSpace?: "normal" | "nowrap"
  textOverflow?: "ellipsis" | "ellipsis-start"
  textDecoration?: "underline" | "line-through" | "none"
  lineClamp?: number

  overflow?: string
  overflowX?: string
  overflowY?: string
  cursor?: CursorValue
  pointerEvents?: "auto" | "none"
  userSelect?: "text" | "none" | "auto"
  selectionColor?: string

  /** Native pseudo-state styles; nesting pseudo states is not supported. */
  hover?: Omit<StyleDesc, "hover" | "active">
  active?: Omit<StyleDesc, "hover" | "active">
}

export interface SyntaxTheme {
  comment?: string
  keyword?: string
  string?: string
  stringSpecial?: string
  escape?: string
  number?: string
  boolean?: string
  typeName?: string
  typeBuiltin?: string
  constructor?: string
  function?: string
  functionBuiltin?: string
  macroName?: string
  property?: string
  constant?: string
  variable?: string
  variableSpecial?: string
  parameter?: string
  operator?: string
  punctuation?: string
  tag?: string
  attribute?: string
  label?: string
  invalid?: string
}

export interface GpuiMetrics {
  codeTextSize?: number
  codeLineHeight?: number
  codeGutterDigitWidth?: number
  codeGutterPaddingRight?: number
  codeGutterMinWidth?: number
  diffTextSize?: number
  diffLineHeight?: number
  diffFileHeaderHeight?: number
  diffHunkHeaderHeight?: number
  diffNoticeHeight?: number
  diffBodyBottomPad?: number
  diffGutterWidth?: number
  diffGutterDigitWidth?: number
  diffGutterPaddingRight?: number
  diffGutterGapLeft?: number
  diffMarkerWidth?: number
  diffAccentBarWidth?: number
  diffRowPaddingX?: number
  diffHeaderGap?: number
  diffHeaderPaddingX?: number
  diffChromeTextSize?: number
  diffMetaTextSize?: number
  diffContentPaddingLeft?: number
  diffWordRadius?: number
  mdTextSize?: number
  mdLineHeight?: number
  mdBlockGap?: number
  mdHeadingSizes?: number[]
  mdHeadingLineHeights?: number[]
  mdTableCellPadding?: number
  mdTableMinColumnWidth?: number
  mdTableMinColumnContent?: number
  mdInlineCodeRadius?: number
  mdQuoteBorderWidth?: number
  mdQuoteRadius?: number
  mdQuotePaddingLeft?: number
  mdQuotePaddingRight?: number
  mdQuotePaddingY?: number
  mdQuoteGap?: number
  mdListGap?: number
  mdListMarkerWidth?: number
  mdListMarkerSize?: number
  mdListMarkerMarginLeft?: number
  mdListRowGap?: number
  mdListItemGap?: number
  mdRuleHeight?: number
  mdCodePaddingX?: number
  mdCodePaddingY?: number
  mdCodeRadius?: number
  mdCodeHeaderPaddingY?: number
  mdCodeHeaderTextSize?: number
}

export interface GpuiTheme {
  appearance?: "dark" | "light"
  bg?: string
  border?: string
  text?: string
  textMuted?: string
  textFaint?: string
  textDim?: string
  accent?: string
  caret?: string
  codeText?: string
  codeWash?: string
  diffAdd?: string
  diffDel?: string
  diffHunkBg?: string
  fontSans?: string
  fontMono?: string
  syntax?: SyntaxTheme
  metrics?: GpuiMetrics
}

/** One native text-search/highlight declaration. */
export interface HighlightSpec {
  query?: string
  caseSensitive?: boolean
  wholeWord?: boolean
  /** Explicit `[start, end)` pairs in UTF-16 code units. */
  ranges?: Array<[number, number]>
  color?: string
  activeColor?: string
  activeIndex?: number
  /** Number of matches before a virtualized subtree. */
  matchIndexOffset?: number
  radius?: number
}

/** One highlight wash painted in the last frame. */
export type HighlightMatch = GeneratedHighlightMatch

export type GpuiEventHandler = (event: EventPayload) => void

export interface HostProps {
  role?: string
  "aria-id"?: string
  "aria-label"?: string
  "aria-description"?: string
  "aria-valuetext"?: string
  "aria-expanded"?: boolean
  "aria-selected"?: boolean
  "aria-level"?: number
  /** Vue reconciliation key; never forwarded to the native element. */
  key?: PropertyKey
  /** Template/component ref; consumed by Vue and never forwarded. */
  ref?: VNodeRef
  style?: StyleDesc
  class?: string
  className?: string
  onClick?: GpuiEventHandler
  onAuxClick?: GpuiEventHandler
  onMouseDown?: GpuiEventHandler
  onMouseUp?: GpuiEventHandler
  onMouseEnter?: GpuiEventHandler
  onMouseLeave?: GpuiEventHandler
  onMouseMove?: GpuiEventHandler
  onMouseDownOutside?: GpuiEventHandler
  onKeyDown?: GpuiEventHandler
  onKeyUp?: GpuiEventHandler
  onFocus?: GpuiEventHandler
  onBlur?: GpuiEventHandler
  onScroll?: GpuiEventHandler
  onChange?: GpuiEventHandler
  onSubmit?: GpuiEventHandler
  onToggleFile?: GpuiEventHandler
  onShowMore?: GpuiEventHandler
  onLineClick?: GpuiEventHandler
  onLinkClick?: GpuiEventHandler
  onVisibleRange?: GpuiEventHandler
  onHighlight?: GpuiEventHandler
  highlight?: HighlightSpec | HighlightSpec[] | null
  autoFocus?: boolean
  tabIndex?: number
  testId?: string
  motion?: MotionProps
}

export interface InputProps extends HostProps {
  value?: string
  placeholder?: string
  readOnly?: boolean
  theme?: GpuiTheme
}

export interface TextareaProps extends InputProps {
  minRows?: number
  maxRows?: number
}

interface VirtualListShared extends HostProps {
  alignment?: "top" | "bottom"
  followTail?: boolean
  overdraw?: number
}

/** A variable-height list that builds only rows near its viewport. */
export type VirtualListProps =
  | (VirtualListShared & {
      estimatedItemHeight?: number
      itemCount?: never
      windowStart?: never
    })
  | (VirtualListShared & {
      itemCount: number
      estimatedItemHeight: number
      windowStart?: number
    })

export interface ImgProps extends HostProps {
  src?: string
  objectFit?: "fill" | "contain" | "cover" | "scaleDown" | "none"
  alt?: string
}

export interface SvgProps extends HostProps {
  src?: string
  /** Raw SVG markup rendered directly by GPUI. */
  source?: string
}

export type CanvasPathOperation =
  | { op: "moveTo"; x: number; y: number }
  | { op: "lineTo"; x: number; y: number }
  | { op: "quadraticTo"; cx: number; cy: number; x: number; y: number }
  | {
      op: "bezierTo"
      cp1x: number
      cp1y: number
      cp2x: number
      cp2y: number
      x: number
      y: number
    }
  | {
      op: "arcTo"
      rx: number
      ry: number
      rotation?: number
      largeArc?: boolean
      sweep?: boolean
      x: number
      y: number
    }
  | { op: "close" }

interface CanvasPaint {
  fill?: string
  stroke?: string
  strokeWidth?: number
}

export type CanvasCommand =
  | (CanvasPaint & {
      type: "path"
      operations: readonly CanvasPathOperation[]
      dash?: readonly number[]
    })
  | (CanvasPaint & {
      type: "rect"
      x: number
      y: number
      width: number
      height: number
      radius?: number
    })
  | (CanvasPaint & {
      type: "circle"
      cx: number
      cy: number
      radius: number
    })
  | {
      type: "line"
      x1: number
      y1: number
      x2: number
      y2: number
      stroke: string
      strokeWidth?: number
      dash?: readonly number[]
    }
  | (CanvasPaint & {
      type: "polyline"
      points: readonly (readonly [number, number])[]
      closed?: boolean
      dash?: readonly number[]
    })

/** Retained GPU drawing. Commands are tessellated only when this prop changes. */
export interface CanvasProps extends HostProps {
  commands?: readonly CanvasCommand[]
}

export interface CodeProps extends HostProps {
  code?: string
  language?: string
  path?: string
  showLineNumbers?: boolean
  theme?: GpuiTheme
}

export interface DiffProps extends HostProps {
  patch?: string
  wordDiff?: boolean
  collapsedPaths?: string[]
  /** Native list virtualization is enabled by default; set false for parent-owned scrolling. */
  scroll?: boolean
  maxLines?: number
  theme?: GpuiTheme
}

export interface MarkdownProps extends HostProps {
  source?: string
  theme?: GpuiTheme
}

export interface AnchoredProps extends HostProps {
  position?: { x: number; y: number }
  side?: "top" | "right" | "bottom" | "left"
  align?: "start" | "center" | "end"
  gap?: number
  anchor?:
    | "topLeft"
    | "topCenter"
    | "topRight"
    | "rightCenter"
    | "bottomRight"
    | "bottomCenter"
    | "bottomLeft"
    | "leftCenter"
  offset?: { x: number; y: number }
  fit?: "switch" | "snap"
  snapMargin?: number
  deferred?: boolean
  priority?: number
  occlude?: boolean
}

export type GpuiElementType =
  | "div"
  | "text"
  | "img"
  | "svg"
  | "canvas"
  | "input"
  | "textarea"
  | "anchored"
  | "code"
  | "diff"
  | "markdown"
  | "virtual-list"

export type DebugFrameOverlayMode = "hidden" | "minimal" | "full"

export type EdgeInsets = GeneratedEdgeInsets
export type NativeWindowInsets = GeneratedWindowInsets
export type DebugFrameOverlayStats = GeneratedDebugFrameOverlayStats

export type WindowOptions = NativeWindowOptions & {
  onEvent?: (event: EventPayload) => void
  debugFrameOverlay?: DebugFrameOverlayMode
}

export interface GpuiIntrinsicElements {
  div: HostProps
  text: HostProps
  img: ImgProps
  svg: SvgProps
  canvas: CanvasProps
  input: InputProps
  textarea: TextareaProps
  anchored: AnchoredProps
  code: CodeProps
  diff: DiffProps
  markdown: MarkdownProps
  "virtual-list": VirtualListProps
}
