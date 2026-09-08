<script setup lang="ts">
import {
  GpuiCode,
  GpuiDiff,
  GpuiMarkdown,
  GpuiTextarea,
  GpuiVirtualList,
  MotionDiv,
  nextTick,
  useElementRef,
  useGpuiWindow,
  type EventPayload,
  type StyleDesc,
} from "@gpui-native/vue"
import { computed, ref } from "vue"

import ChatIcon from "./ChatIcon.vue"
import ChipPicker from "./ChipPicker.vue"
import {
  accessOptions,
  branchOptions,
  chatTheme,
  colors,
  CONTENT_MAX_WIDTH,
  conversations,
  expandTurns,
  models,
  projectOptions,
  reasoningOptions,
  SIDEBAR_WIDTH,
  TITLEBAR_HEIGHT,
  workspaceOptions,
  type Conversation,
  type Turn,
} from "./data.js"
import type { IconName } from "./icons.js"

const TRAFFIC_LIGHT_CLEARANCE = globalThis.navigator?.platform.includes("Mac") ? 86 : 8
const INITIAL_TURN_COUNT = 5_000

const activeId = ref("c1")
const collapsed = ref(false)
const draft = ref("")
const model = ref("deepseek-v4-flash")
const reasoning = ref("high")
const access = ref("full")
const mode = ref<"build" | "plan">("build")
const project = ref("waku")
const workspace = ref("local")
const branch = ref("main")
const copied = ref(false)
const feedback = ref<"up" | "down" | null>(null)
const turns = ref<Turn[]>(expandTurns(INITIAL_TURN_COUNT))

const transcriptRef = useElementRef()
const gpuiWindow = useGpuiWindow()
const title = computed(
  () => conversations.find((conversation) => conversation.id === activeId.value)?.title ?? "",
)
const groups = computed(() => {
  const result: { name: string; items: Conversation[] }[] = []
  for (const conversation of conversations) {
    const last = result.at(-1)
    if (last?.name === conversation.group) last.items.push(conversation)
    else result.push({ name: conversation.group, items: [conversation] })
  }
  return result
})

const rootStyle: StyleDesc = {
  display: "flex",
  flexDirection: "row",
  width: "100%",
  height: "100%",
  fontFamily: ".SystemUIFont",
  color: colors.text,
}
const sidebarClipStyle: StyleDesc = {
  display: "flex",
  flexDirection: "row",
  height: "100%",
  flexShrink: 0,
  overflow: "hidden",
}
const sidebarStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  width: SIDEBAR_WIDTH,
  flexShrink: 0,
  height: "100%",
  background: colors.sidebar,
  userSelect: "none",
}
const contentStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  flexGrow: 1,
  minWidth: 0,
  height: "100%",
  background: colors.canvas,
}
const transcriptStyle: StyleDesc = {
  flexGrow: 1,
  minHeight: 0,
  width: "100%",
}
const rowInnerStyle: StyleDesc = { width: CONTENT_MAX_WIDTH, maxWidth: "100%" }
const composerShellStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  flexShrink: 0,
  paddingLeft: 20,
  paddingRight: 20,
  overflow: "visible",
  userSelect: "none",
}
const composerStyle: StyleDesc = {
  display: "flex",
  flexDirection: "column",
  width: "100%",
  maxWidth: CONTENT_MAX_WIDTH,
  overflow: "visible",
  background: colors.composer,
  borderRadius: 13,
  borderWidth: 1,
  borderColor: colors.border,
  paddingTop: 10,
  paddingBottom: 10,
}

const introMarkdown = `# Vue-composed chat

This Waku-style desktop is now rendered by **Vue 3**, GPUI, and Rust—without Electron.

- 5,000 virtualized transcript rows
- Native selectable Markdown, code, and diffs
- Vue state driving the sidebar, composer, menus, and motion`

function rowStyle(index: number): StyleDesc {
  return {
    display: "flex",
    flexDirection: "row",
    justifyContent: "center",
    width: "100%",
    paddingTop: index === 0 ? 22 : 8,
    paddingBottom: index === turns.value.length - 1 ? 22 : 8,
    paddingLeft: 20,
    paddingRight: 20,
  }
}

function iconButtonStyle(dimmed = false): StyleDesc {
  return {
    width: 26,
    height: 26,
    flexShrink: 0,
    borderRadius: 6,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    opacity: dimmed ? 0.35 : 1,
    ...(dimmed
      ? {}
      : {
          cursor: "pointer" as const,
          hover: { background: colors.overlay },
          active: { background: colors.overlayStrong },
        }),
  }
}

function sidebarActionStyle(): StyleDesc {
  return {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
    height: 32,
    paddingLeft: 4,
    paddingRight: 4,
    borderRadius: 7,
    cursor: "pointer",
    hover: { background: colors.item },
    active: { background: colors.overlayStrong },
  }
}

function conversationStyle(conversation: Conversation): StyleDesc {
  return {
    display: "flex",
    flexDirection: "column",
    gap: 4,
    paddingLeft: 8,
    paddingRight: 8,
    paddingTop: 7,
    paddingBottom: 7,
    borderRadius: 7,
    cursor: "pointer",
    background: conversation.id === activeId.value ? colors.item : "#00000000",
    hover: { background: colors.item },
  }
}

function turnKind(turn: Turn): IconName {
  return turn.kind === "diff" ? "gitBranch" : turn.kind === "code" ? "wrench" : "sparkle"
}

function toggleFeedback(next: "up" | "down"): void {
  feedback.value = feedback.value === next ? null : next
}

function ghostButtonStyle(active = false, label = false): StyleDesc {
  return {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    height: 30,
    paddingLeft: label ? 9 : 0,
    paddingRight: label ? 11 : 0,
    ...(label ? {} : { width: 30 }),
    justifyContent: "center",
    borderRadius: 10,
    cursor: "pointer",
    background: active ? colors.overlayStrong : "#00000000",
    hover: { background: colors.overlay },
  }
}

function sendButtonStyle(): StyleDesc {
  const ready = draft.value.trim().length > 0
  return {
    width: 26,
    height: 26,
    borderRadius: 13,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    background: ready ? colors.inverse : colors.overlayStrong,
    ...(ready ? { cursor: "pointer" as const, hover: { opacity: 0.9 } } : {}),
  }
}

async function sendDraft(event?: EventPayload): Promise<void> {
  const next = (event?.value ?? draft.value).trim()
  if (next.length === 0) return
  turns.value.push({ id: `user-${Date.now()}-${turns.value.length}`, kind: "user", text: next })
  draft.value = ""
  await nextTick()
  if (transcriptRef.value !== null) gpuiWindow.scrollToItem(transcriptRef.value, turns.value.length)
}
</script>

<template>
  <div :style="rootStyle">
    <MotionDiv
      :initial="false"
      :animate="{ width: collapsed ? 0 : SIDEBAR_WIDTH + 1 }"
      :transition="{ duration: 0.2, ease: 'easeOut' }"
      :style="sidebarClipStyle"
    >
      <div :style="sidebarStyle">
        <div
          :style="{
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            height: TITLEBAR_HEIGHT,
            flexShrink: 0,
          }"
        >
          <div :style="{ width: TRAFFIC_LIGHT_CLEARANCE, height: '100%', flexShrink: 0 }" />
          <div testId="sidebar-collapse" :style="iconButtonStyle()" @click="collapsed = true">
            <ChatIcon name="sidebar" :size="16" :color="colors.tertiary" />
          </div>
          <div
            :style="{
              display: 'flex',
              flexDirection: 'row',
              alignItems: 'center',
              gap: 2,
              marginLeft: 6,
            }"
          >
            <div :style="iconButtonStyle(true)">
              <ChatIcon name="arrowLeft" :color="colors.tertiary" />
            </div>
            <div :style="iconButtonStyle(true)">
              <ChatIcon name="arrowRight" :color="colors.tertiary" />
            </div>
          </div>
        </div>

        <div
          :style="{ display: 'flex', flexDirection: 'column', paddingLeft: 10, paddingRight: 10 }"
        >
          <div :style="sidebarActionStyle()">
            <div
              :style="{
                width: 20,
                height: 20,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }"
            >
              <ChatIcon name="compose" :color="colors.secondary" />
            </div>
            <text :style="{ fontSize: 13, color: colors.secondary }">New Task</text>
          </div>
        </div>

        <div
          :style="{
            display: 'flex',
            flexDirection: 'column',
            flexGrow: 1,
            minHeight: 0,
            overflowY: 'scroll',
            paddingLeft: 10,
            paddingRight: 10,
          }"
        >
          <div :style="{ paddingBottom: 6 }">
            <div :style="sidebarActionStyle()">
              <div
                :style="{
                  width: 20,
                  height: 20,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }"
              >
                <ChatIcon name="search" :color="colors.secondary" />
              </div>
              <text :style="{ fontSize: 13, color: colors.secondary }">Search</text>
            </div>
          </div>

          <div
            v-for="(group, groupIndex) in groups"
            :key="group.name"
            :style="{ display: 'flex', flexDirection: 'column', paddingBottom: 10 }"
          >
            <div
              :style="{
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                height: 28,
                paddingLeft: 8,
                paddingRight: 8,
              }"
            >
              <text
                :style="{
                  fontSize: 13,
                  fontWeight: 500,
                  color: colors.secondary,
                  flexGrow: 1,
                  minWidth: 0,
                }"
              >
                {{ group.name }}
              </text>
              <ChatIcon
                v-if="groupIndex === 0"
                name="listFilter"
                :size="14"
                :color="colors.secondary"
              />
            </div>
            <div
              v-for="conversation in group.items"
              :key="conversation.id"
              :style="conversationStyle(conversation)"
              @click="activeId = conversation.id"
            >
              <text
                :style="{
                  fontSize: 13.5,
                  lineHeight: 18,
                  color: colors.text,
                  whiteSpace: 'nowrap',
                  textOverflow: 'ellipsis',
                }"
              >
                {{ conversation.title }}
              </text>
              <div
                :style="{
                  display: 'flex',
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 5,
                }"
              >
                <ChatIcon name="folder" :size="12.5" :color="colors.tertiary" />
                <text
                  :style="{
                    fontSize: 13,
                    lineHeight: 15,
                    color: colors.tertiary,
                    flexGrow: 1,
                    minWidth: 0,
                    whiteSpace: 'nowrap',
                    textOverflow: 'ellipsis',
                  }"
                >
                  {{ conversation.project }}
                </text>
                <text :style="{ fontSize: 12.5, color: colors.ghost, flexShrink: 0 }">
                  {{ conversation.time }}
                </text>
              </div>
            </div>
          </div>
        </div>

        <div
          :style="{
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            height: 40,
            flexShrink: 0,
            paddingLeft: 10,
            paddingRight: 10,
          }"
        >
          <div :style="iconButtonStyle()">
            <ChatIcon name="settings" :color="colors.tertiary" />
          </div>
        </div>
      </div>
      <div
        :style="{
          width: 1,
          height: '100%',
          flexShrink: 0,
          background: colors.sidebarBorder,
        }"
      />
    </MotionDiv>

    <div :style="contentStyle">
      <div
        :style="{
          display: 'flex',
          flexDirection: 'row',
          alignItems: 'center',
          gap: 8,
          height: TITLEBAR_HEIGHT,
          flexShrink: 0,
          paddingLeft: collapsed ? 0 : 14,
          paddingRight: 14,
          userSelect: 'none',
        }"
      >
        <template v-if="collapsed">
          <div :style="{ width: TRAFFIC_LIGHT_CLEARANCE - 8, height: '100%', flexShrink: 0 }" />
          <div testId="sidebar-expand" :style="iconButtonStyle()" @click="collapsed = false">
            <ChatIcon name="sidebar" :size="16" :color="colors.tertiary" />
          </div>
          <div :style="iconButtonStyle(true)">
            <ChatIcon name="arrowLeft" :color="colors.tertiary" />
          </div>
          <div :style="iconButtonStyle(true)">
            <ChatIcon name="arrowRight" :color="colors.tertiary" />
          </div>
        </template>
        <text
          :style="{
            fontSize: 13,
            fontWeight: 500,
            color: colors.text,
            whiteSpace: 'nowrap',
            textOverflow: 'ellipsis',
            minWidth: 0,
            flexShrink: 1,
          }"
        >
          {{ title }}
        </text>
        <text :style="{ fontSize: 12, color: colors.tertiary, flexShrink: 0 }">
          {{ turns.length.toLocaleString("en-US") }} messages
        </text>
        <div :style="{ flexGrow: 1 }" />
        <div :style="iconButtonStyle()">
          <ChatIcon name="panelRight" :color="colors.tertiary" />
        </div>
      </div>

      <GpuiVirtualList
        ref="transcriptRef"
        :overdraw="240"
        :estimated-item-height="220"
        :style="transcriptStyle"
      >
        <div :style="rowStyle(0)">
          <div :style="rowInnerStyle">
            <div
              :style="{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'flex-end',
                width: '100%',
                paddingBottom: 12,
              }"
            >
              <div
                :style="{
                  maxWidth: 540,
                  background: colors.raised,
                  borderRadius: 12,
                  padding: 10,
                  paddingLeft: 12,
                  paddingRight: 12,
                }"
              >
                <text :style="{ fontSize: 14, lineHeight: 20, color: colors.text }">
                  Can this chat be rendered with Vue instead?
                </text>
              </div>
            </div>
            <GpuiMarkdown :source="introMarkdown" :theme="chatTheme" />
            <div
              :style="{
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                gap: 4,
                paddingTop: 6,
                marginLeft: -7,
                userSelect: 'none',
              }"
            >
              <div :style="ghostButtonStyle(copied)" @click="copied = !copied">
                <ChatIcon
                  :name="copied ? 'check' : 'copy'"
                  :size="16"
                  :color="copied ? colors.text : colors.ghost"
                />
              </div>
              <div :style="ghostButtonStyle(feedback === 'up')" @click="toggleFeedback('up')">
                <ChatIcon
                  name="thumbsUp"
                  :size="16"
                  :color="feedback === 'up' ? colors.text : colors.ghost"
                />
              </div>
              <div :style="ghostButtonStyle(feedback === 'down')" @click="toggleFeedback('down')">
                <ChatIcon
                  name="thumbsDown"
                  :size="16"
                  :color="feedback === 'down' ? colors.text : colors.ghost"
                />
              </div>
              <div
                v-for="icon in ['retry', 'share', 'more'] as IconName[]"
                :key="icon"
                :style="ghostButtonStyle()"
              >
                <ChatIcon :name="icon" :size="16" :color="colors.ghost" />
              </div>
            </div>
          </div>
        </div>

        <div v-for="(turn, index) in turns" :key="turn.id" :style="rowStyle(Number(index) + 1)">
          <div :style="rowInnerStyle">
            <div
              v-if="turn.kind === 'user'"
              :style="{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'flex-end',
                width: '100%',
              }"
            >
              <div
                :style="{
                  maxWidth: 540,
                  background: colors.raised,
                  borderRadius: 12,
                  paddingTop: 8,
                  paddingBottom: 8,
                  paddingLeft: 12,
                  paddingRight: 12,
                }"
              >
                <text :style="{ fontSize: 14, lineHeight: 20, color: colors.text }">
                  {{ turn.text }}
                </text>
              </div>
            </div>

            <div
              v-else-if="turn.kind === 'fold'"
              :style="{
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                gap: 10,
                height: 24,
                width: '100%',
              }"
            >
              <div :style="{ height: 1, flexGrow: 1, background: colors.border }" />
              <text :style="{ fontSize: 13.5, fontWeight: 500, color: colors.tertiary }">
                {{ turn.duration }}
              </text>
              <ChatIcon name="chevronRight" :size="11.5" :color="colors.tertiary" />
              <div :style="{ height: 1, flexGrow: 1, background: colors.border }" />
            </div>

            <GpuiMarkdown
              v-else-if="turn.kind === 'markdown'"
              :source="turn.source"
              :theme="chatTheme"
            />
            <div
              v-else-if="turn.kind === 'code'"
              :style="{
                display: 'flex',
                flexDirection: 'column',
                background: '#141414',
                borderRadius: 10,
                borderWidth: 1,
                borderColor: colors.border,
                overflow: 'hidden',
              }"
            >
              <div
                :style="{
                  display: 'flex',
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: 7,
                  height: 32,
                  paddingLeft: 12,
                  paddingRight: 12,
                  background: colors.raised,
                }"
              >
                <ChatIcon :name="turnKind(turn)" :size="12" :color="colors.tertiary" />
                <text :style="{ fontSize: 12, color: colors.secondary }">{{ turn.language }}</text>
              </div>
              <GpuiCode
                :code="turn.source"
                :language="turn.language"
                show-line-numbers
                :theme="chatTheme"
                :style="{ padding: 10 }"
              />
            </div>
            <GpuiDiff
              v-else-if="turn.kind === 'diff'"
              :patch="turn.patch"
              word-diff
              :theme="chatTheme"
            />
          </div>
        </div>
      </GpuiVirtualList>

      <div :style="composerShellStyle">
        <div :style="composerStyle">
          <GpuiTextarea
            v-model="draft"
            testId="composer"
            placeholder="Do anything..."
            :min-rows="1"
            :max-rows="3"
            auto-focus
            :theme="chatTheme"
            :style="{
              width: '100%',
              minWidth: 0,
              fontSize: 14,
              lineHeight: 20,
              color: colors.text,
              background: '#00000000',
              borderWidth: 0,
              paddingLeft: 10,
              paddingRight: 10,
            }"
            @submit="sendDraft"
          />
          <div
            :style="{
              display: 'flex',
              flexDirection: 'row',
              alignItems: 'center',
              gap: 4,
              marginTop: 8,
              paddingLeft: 10,
              paddingRight: 10,
            }"
          >
            <ChipPicker v-model="model" :options="models" />
            <ChipPicker v-model="reasoning" :options="reasoningOptions" :caret="false" />
            <ChipPicker
              v-model="access"
              :options="accessOptions"
              :caret="false"
              :menu-width="288"
            />
            <div
              :style="{
                display: 'flex',
                flexDirection: 'row',
                alignItems: 'center',
                gap: 6,
                height: 26,
                paddingLeft: 7,
                paddingRight: 7,
                borderRadius: 6,
                cursor: 'pointer',
                hover: { background: colors.overlay },
              }"
              @click="mode = mode === 'plan' ? 'build' : 'plan'"
            >
              <ChatIcon
                :name="mode === 'plan' ? 'list' : 'wrench'"
                :size="12"
                :color="mode === 'plan' ? colors.accent : colors.tertiary"
              />
              <text
                :style="{
                  fontSize: 13,
                  color: mode === 'plan' ? colors.accent : colors.secondary,
                }"
              >
                {{ mode === "plan" ? "Plan" : "Build" }}
              </text>
            </div>
            <div :style="{ flexGrow: 1 }" />
            <div testId="send" :style="sendButtonStyle()" @click="sendDraft()">
              <ChatIcon
                name="send"
                :size="16"
                :color="draft.trim().length > 0 ? colors.onInverse : colors.ghost"
              />
            </div>
          </div>
        </div>
      </div>

      <div
        :style="{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          flexShrink: 0,
          paddingLeft: 20,
          paddingRight: 20,
          paddingTop: 4,
          paddingBottom: 8,
          userSelect: 'none',
        }"
      >
        <div
          :style="{
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            gap: 2,
            width: '100%',
            maxWidth: CONTENT_MAX_WIDTH,
            height: 28,
            paddingLeft: 10,
            paddingRight: 10,
          }"
        >
          <ChipPicker v-model="project" :options="projectOptions" :caret="false" />
          <ChipPicker v-model="workspace" :options="workspaceOptions" :caret="false" />
          <ChipPicker v-if="project !== 'none'" v-model="branch" :options="branchOptions" />
          <div :style="{ flexGrow: 1 }" />
          <div
            :style="{
              width: 8,
              height: 8,
              borderRadius: 4,
              background: '#3B82F6',
              flexShrink: 0,
            }"
          />
        </div>
      </div>
    </div>
  </div>
</template>
