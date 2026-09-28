<script setup lang="ts">
import type { ProfileInfo } from "../types";

defineProps<{
  profiles: ProfileInfo[];
  current: string | null;
  pathText: string;
}>();

const emit = defineEmits<{
  select: [name: string];
}>();
</script>

<template>
  <aside id="sidebar">
    <div class="side-title">战役档案（存档位）</div>
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
        <div class="profile-name">{{ p.name }}</div>
        <div class="profile-week">当前：{{ p.week != null ? `第 ${p.week} 周` : "未知" }}</div>
      </div>
    </div>
    <div class="side-path">{{ pathText }}</div>
  </aside>
</template>
