import type * as Shared from "@gpui-native/runtime/types"
import type { VNodeRef } from "@vue/runtime-core"
export * from "@gpui-native/runtime/types"
export type HostProps = Shared.HostProps & { ref?: VNodeRef }
export type ImgProps = Shared.ImgProps & { ref?: VNodeRef }
export type SvgProps = Shared.SvgProps & { ref?: VNodeRef }
export type CanvasProps = Shared.CanvasProps & { ref?: VNodeRef }
export type AnchoredProps = Shared.AnchoredProps & { ref?: VNodeRef }
export type CodeProps = Shared.CodeProps & { ref?: VNodeRef }
export type DiffProps = Shared.DiffProps & { ref?: VNodeRef }
export type MarkdownProps = Shared.MarkdownProps & { ref?: VNodeRef }
export type InputProps = Shared.InputProps & { ref?: VNodeRef }
export type TextareaProps = Shared.TextareaProps & { ref?: VNodeRef }
export type VirtualListProps = Shared.VirtualListProps & { ref?: VNodeRef }

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
