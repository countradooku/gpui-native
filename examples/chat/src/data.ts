import type { IconName } from "./icons.js"

export const colors = {
  canvas: "#1A1A1A",
  sidebar: "#181818",
  raised: "#232323",
  composer: "#212121",
  overlay: "#E6EAF20D",
  overlayStrong: "#E6EAF217",
  item: "#F0F0F00F",
  border: "#E6EAF212",
  borderStrong: "#E6EAF224",
  sidebarBorder: "#292929",
  text: "#E2E2E2",
  secondary: "#A3A3A3",
  tertiary: "#7D7D7D",
  ghost: "#575757",
  accent: "#E2795B",
  inverse: "#E7E9EC",
  onInverse: "#17181C",
  codeText: "#E0A882",
} as const

export const SIDEBAR_WIDTH = 252
export const CONTENT_MAX_WIDTH = 720
export const TITLEBAR_HEIGHT = 48

export const chatTheme = {
  text: colors.text,
  textMuted: colors.secondary,
  textFaint: colors.tertiary,
  textDim: colors.secondary,
  border: colors.border,
  bg: colors.canvas,
  accent: colors.accent,
  caret: colors.accent,
  fontSans: ".SystemUIFont",
  codeText: colors.codeText,
  codeWash: "#E6EAF214",
  metrics: {
    mdTextSize: 14,
    mdLineHeight: 22,
    mdBlockGap: 14,
    mdHeadingSizes: [20, 16, 14, 14],
    mdHeadingLineHeights: [28, 24, 22, 22],
    mdCodePaddingX: 12,
    mdCodePaddingY: 10,
    mdCodeRadius: 10,
    mdCodeHeaderHeight: 0,
    codeTextSize: 12.5,
    codeLineHeight: 20,
    diffLineHeight: 20,
    diffFileHeaderHeight: 34,
  },
}

export interface Conversation {
  id: string
  title: string
  group: string
  project: string
  time: string
}

export interface PickerOption {
  id: string
  label: string
  icon: IconName
}

export const models = [
  { id: "deepseek-v4-flash", label: "DeepSeek V4 Flash", icon: "sparkle" },
  { id: "deepseek-v4", label: "DeepSeek V4", icon: "sparkle" },
  { id: "opus-4.6", label: "Claude Opus 4.6", icon: "sparkle" },
  { id: "sonnet-4.6", label: "Claude Sonnet 4.6", icon: "sparkle" },
  { id: "gpt-5.4", label: "GPT-5.4", icon: "sparkle" },
  { id: "grok-4", label: "Grok 4", icon: "sparkle" },
] satisfies PickerOption[]

export const reasoningOptions = [
  { id: "high", label: "High", icon: "sparkle" },
  { id: "medium", label: "Medium", icon: "sparkle" },
  { id: "low", label: "Low", icon: "zap" },
] satisfies PickerOption[]

export const accessOptions = [
  { id: "ask", label: "Supervised", icon: "lock" },
  { id: "edits", label: "Auto-accept edits", icon: "pencil" },
  { id: "auto", label: "Auto", icon: "sparkle" },
  { id: "full", label: "Full access", icon: "lockOpen" },
] satisfies PickerOption[]

export const projectOptions = [
  { id: "waku", label: "waku", icon: "folder" },
  { id: "gpuix", label: "gpuix", icon: "folder" },
  { id: "none", label: "No project", icon: "folder" },
] satisfies PickerOption[]

export const workspaceOptions = [
  { id: "local", label: "Local", icon: "laptop" },
  { id: "worktree", label: "New worktree", icon: "gitBranch" },
] satisfies PickerOption[]

export const branchOptions = [
  { id: "main", label: "main", icon: "gitBranch" },
  { id: "feat-selectors", label: "feat/selectors", icon: "gitBranch" },
  { id: "waku-clone", label: "waku-clone", icon: "gitBranch" },
] satisfies PickerOption[]

export const conversations: Conversation[] = [
  { id: "c1", title: "give me a quick overview", group: "Yesterday", project: "waku", time: "16m" },
  {
    id: "c2",
    title: "Native SDK vs GPUI comparison",
    group: "Yesterday",
    project: "No project",
    time: "14h",
  },
  {
    id: "c3",
    title: "Vercel Labs scriptc implementat...",
    group: "Yesterday",
    project: "No project",
    time: "15h",
  },
  {
    id: "c4",
    title: "check if any memory optimizatio...",
    group: "This Month",
    project: "waku",
    time: "2d",
  },
]

const overview =
  "**Waku** is a native control plane for local coding agents. Rust plus GPUI. One window, no Electron."
const architecture =
  "The desktop is an RPC client. The daemon owns provider sessions over a WebSocket."
const selection =
  "Selection is rebuilt from the paint pass. Each string registers in document order, so a drag can cross elements."
const selectionCode = `pub fn resolve_spans(
    elements: &[(&str, &str)],
    a: (usize, usize),
    b: (usize, usize),
) -> Vec<Span> {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    let mut spans = Vec::new();
    for (ei, (key, text)) in elements.iter().enumerate().take(end.0 + 1).skip(start.0) {
        let from = if ei == start.0 { start.1 } else { 0 };
        let to = if ei == end.0 { end.1 } else { text.len() };
        if from < to {
            spans.push(Span { key: key.to_string(), range: from..to });
        }
    }
    spans
}`
const gutter =
  "The gutter width now follows the largest line number, so a five-digit line no longer hits the accent bar."
const gutterDiff = [
  "diff --git a/packages/native/src/diff/mod.rs b/packages/native/src/diff/mod.rs",
  "index 8f2a1c4..d91b7e0 100644",
  "--- a/packages/native/src/diff/mod.rs",
  "+++ b/packages/native/src/diff/mod.rs",
  "@@ -78,12 +78,15 @@ impl FileDiff {",
  " /// Width of one line-number gutter, fitted to the largest line number.",
  "-pub fn gutter_width(file: &FileDiff) -> f32 {",
  "-    GUTTER_WIDTH",
  "+pub fn gutter_width(file: &FileDiff, metrics: &Metrics) -> f32 {",
  "+    let digits = file.max_line.max(1).ilog10() + 1;",
  "+    (digits as f32 * 6.6 + 8.0 + 6.0).max(metrics.diff_gutter_width)",
  " }",
].join("\n")

export type Turn =
  | { kind: "user"; text: string }
  | { kind: "fold"; duration: string }
  | { kind: "markdown"; source: string }
  | { kind: "code"; language: string; source: string }
  | { kind: "diff"; patch: string }

export const baseTurns: Turn[] = [
  { kind: "user", text: "give me a quick overview" },
  { kind: "fold", duration: "Worked for 10 seconds" },
  { kind: "markdown", source: overview },
  { kind: "user", text: "How does the daemon split from the desktop?" },
  { kind: "fold", duration: "Worked for 6 seconds" },
  { kind: "markdown", source: architecture },
  { kind: "user", text: "How does cross-element text selection work?" },
  { kind: "fold", duration: "Worked for 14 seconds" },
  { kind: "markdown", source: selection },
  { kind: "code", language: "rust", source: selectionCode },
  { kind: "user", text: "Make the diff gutter width adapt to the largest line number." },
  { kind: "fold", duration: "Worked for 8 seconds" },
  { kind: "markdown", source: gutter },
  { kind: "diff", patch: gutterDiff },
  { kind: "user", text: "Do I get hot reload when I edit the Rust side?" },
  { kind: "fold", duration: "Worked for 4 seconds" },
  { kind: "markdown", source: "**No.** A `.node` cannot unload. The loop rebuilds and restarts." },
  { kind: "user", text: "How do skills show up in the app?" },
  { kind: "fold", duration: "Worked for 7 seconds" },
  {
    kind: "markdown",
    source: "Skills are `SKILL.md` files. A mail-style list on the left, the body on the right.",
  },
  { kind: "user", text: "Which models should I wire up?" },
  { kind: "fold", duration: "Worked for 5 seconds" },
  {
    kind: "markdown",
    source:
      "Default is DeepSeek V4 Flash. Keep Opus for long diffs. Hide GPT-5.4 behind the picker.",
  },
]

export function expandTurns(count: number): Turn[] {
  if (count <= baseTurns.length) return baseTurns.slice(0, count)
  return Array.from({ length: count }, (_, index) => baseTurns[index % baseTurns.length]!)
}
