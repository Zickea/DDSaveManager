<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { NButton, NLayout } from "naive-ui";
import type { ProfileInfo } from "../types";

const props = defineProps<{
  profiles: ProfileInfo[];
  current: string | null;
  pathText: string;
}>();

const emit = defineEmits<{
  select: [name: string];
  "delete-profile": [name: string];
}>();

// 游戏模式英文 → 中文（暗黑地牢1 难度/模式）
const MODE_LABELS: Record<string, string> = {
  radiant: "光辉",
  darkest: "黑暗",
  bloodmoon: "血月",
  stygian: "斯提吉安",
  new_game_plus: "新游戏+",
};

function modeLabel(mode: string | null): string | null {
  if (!mode) return null;
  return MODE_LABELS[mode] ?? mode;
}

// 卡片附加信息："极暗 · 血月" 或 "英雄 4 · 离队 0"
function metaText(p: ProfileInfo): string {
  const parts: string[] = [];
  const estate = p.estate_name ?? "";
  const mode = modeLabel(p.game_mode);
  if (estate) parts.push(estate);
  if (mode) parts.push(mode);
  const roster: string[] = [];
  if (p.hero_count != null) roster.push(`英雄 ${p.hero_count}`);
  if (p.dead_count != null) roster.push(`离队 ${p.dead_count}`);
  const joined = parts.join(" · ");
  const rosterJoined = roster.join(" · ");
  return [joined, rosterJoined].filter(Boolean).join(" ｜ ");
}

// 点击存档目录文本 → 在文件资源管理器中打开（后端只打开自身定位的 remote 目录）
async function openPath() {
  try {
    await invoke("open_remote_dir");
  } catch (e) {
    console.error("open_remote_dir:", e);
  }
}
</script>

<template>
  <n-layout content-style="padding: 16px 14px;" content-class="sider-inner">
    <div class="side-title">
      战役档案（存档位）
    </div>
    <div id="profile-list" class="profile-list">
      <div v-if="profiles.length === 0" class="empty">
        未找到任何档案（profile_N），请先进入游戏创建战役
      </div>
      <div
        v-for="p in profiles"
        :key="p.name"
        class="profile-card"
        :class="{ active: p.name === current }"
        @click="emit('select', p.name)"
      >
        <div class="profile-info">
          <div class="profile-name">{{ p.name }}</div>
          <div class="profile-week">当前：{{ p.week != null ? `第 ${p.week} 周` : "未知" }}</div>
          <div v-if="metaText(p)" class="profile-meta">{{ metaText(p) }}</div>
        </div>
        <n-button
          class="del-btn"
          size="tiny"
          quaternary
          circle
          title="删除此档案"
          @click.stop="emit('delete-profile', p.name)"
        >
          ✕
        </n-button>
      </div>
    </div>
    <div class="side-path" title="点击在文件资源管理器中打开存档目录" @click="openPath">
      {{ pathText }}
    </div>
  </n-layout>
</template>

<style scoped>
.profile-meta {
  font-size: 10px;
  color: var(--text-faint);
  margin-top: 2px;
  line-height: 1.5;
  word-break: break-all;
}
</style>
