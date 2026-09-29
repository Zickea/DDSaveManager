<script setup lang="ts">
import { computed } from "vue";
import { NTag, NLayoutContent } from "naive-ui";
import type { BackupEntry } from "../types";

const props = defineProps<{
  backups: BackupEntry[];
  selected: string | null;
}>();

const emit = defineEmits<{
  "select-backup": [name: string];
}>();

// 按周分组（week 降序，未知周放最后）；组内：auto 第一，其余按时间（名称字典序）降序
const groups = computed(() => {
  const map = new Map<number, BackupEntry[]>();
  for (const b of props.backups) {
    const key = b.week ?? -1;
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(b);
  }
  const weeks = [...map.keys()].sort((a, b) => b - a);
  for (const w of weeks) {
    map.get(w)!.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "auto" ? -1 : 1;
      return b.name.localeCompare(a.name);
    });
  }
  return weeks.map((w) => ({ week: w, items: map.get(w)! }));
});

function weekLabel(w: number): string {
  if (w === -1) return "未知周数";
  if (w === 0) return "第 0 周（教学关）";
  return `第 ${w} 周`;
}

function timeLabel(ts: string): string {
  return ts.replace("_", " ");
}
</script>

<template>
  <n-layout-content content-class="backup-list" :native-scrollbar="false">
    <div v-if="backups.length === 0" class="empty">
      该档案还没有备份。点击「＋ 手动备份」，或启动监控等待回城自动备份
    </div>
    <div v-for="g in groups" :key="g.week" class="week-group">
      <div class="week-header">{{ weekLabel(g.week) }}</div>
      <div
        v-for="b in g.items"
        :key="b.name"
        class="backup-row"
        :class="{ selected: b.name === selected }"
        @click="emit('select-backup', b.name)"
      >
        <n-tag
          :type="b.kind === 'auto' ? 'success' : 'warning'"
          size="small"
          :bordered="false"
        >
          {{ b.kind === "auto" ? "自动" : "手动" }}
        </n-tag>
        <span class="backup-time">{{ timeLabel(b.timestamp) }}</span>
      </div>
    </div>
  </n-layout-content>
</template>
