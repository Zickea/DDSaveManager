<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { NButton, NLayout } from "naive-ui";
import type { ProfileInfo } from "../types";

defineProps<{
  profiles: ProfileInfo[];
  current: string | null;
  pathText: string;
}>();

const emit = defineEmits<{
  select: [name: string];
  "delete-profile": [name: string];
}>();

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
