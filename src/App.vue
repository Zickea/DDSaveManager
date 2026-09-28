<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { BackupEntry, ProfileInfo, StatusInfo } from "./types";
import ProfileSidebar from "./components/ProfileSidebar.vue";
import BackupPanel from "./components/BackupPanel.vue";

const profiles = ref<ProfileInfo[]>([]);
const currentProfile = ref<string | null>(null);
const selectedBackup = ref<string | null>(null);
const gameRunning = ref(false);
const watching = ref<string[]>([]);
const remotePath = ref("");

// 监控状态：事件可临时覆盖显示文本，下一次状态刷新自动还原
const watchStatusOverride = ref<string | null>(null);
const watchStatusKind = ref<"off" | "good" | "warn">("off");
const watchStatusText = computed(() =>
  watchStatusOverride.value ??
  (watching.value.length > 0 ? `监控中：${watching.value.length} 个档案` : "监控未启动"),
);
const watchBtnText = computed(() => (watching.value.length > 0 ? "停止监控" : "启动监控"));

const backups = ref<BackupEntry[]>([]);

/* ========== Toast ========== */
const toastMsg = ref("");
const toastErr = ref(false);
let toastTimer: number | undefined;
function toast(msg: string, isErr = false) {
  toastMsg.value = msg;
  toastErr.value = isErr;
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (toastMsg.value = ""), 3500);
}

/* ========== 状态 ========== */
async function refreshStatus() {
  try {
    const s: StatusInfo = await invoke("get_status");
    remotePath.value = s.remote_dir
      ? `存档目录：${s.remote_dir}\n备份保存在各 profile_N 目录内（与官方 backup 同级）`
      : "存档目录：未找到（请确认已安装并运行过游戏）";
    gameRunning.value = s.game_running;
    watching.value = s.watching;
    watchStatusOverride.value = null;
    watchStatusKind.value = watching.value.length > 0 ? "good" : "off";
  } catch (e) {
    console.error(e);
  }
}

/* ========== 档案 ========== */
const knownProfiles = new Set<string>();
async function refreshProfiles() {
  profiles.value = await invoke<ProfileInfo[]>("get_profiles");
  // 自动发现新档案并接入监控（幂等）
  const fresh = profiles.value.filter((p) => !knownProfiles.has(p.name));
  for (const p of profiles.value) knownProfiles.add(p.name);
  if (fresh.length > 0) {
    try {
      await invoke("start_watchers");
      refreshStatus();
    } catch (e) {
      console.error("start_watchers:", e);
    }
  }
  if (!currentProfile.value && profiles.value.length > 0) {
    selectProfile(profiles.value[0].name);
  }
}

function selectProfile(name: string) {
  currentProfile.value = name;
  selectedBackup.value = null;
  refreshBackups();
}

/* ========== 备份列表 ========== */
async function refreshBackups() {
  if (!currentProfile.value) return;
  backups.value = await invoke<BackupEntry[]>("list_backups", {
    profile: currentProfile.value,
  });
}

function selectBackup(name: string) {
  selectedBackup.value = name === selectedBackup.value ? null : name;
}

/* ========== 动作 ========== */
async function manualBackup() {
  if (!currentProfile.value) return;
  try {
    const entry = await invoke<BackupEntry>("manual_backup", {
      profile: currentProfile.value,
    });
    toast(`已手动备份：${entry.name}`);
    refreshBackups();
  } catch (e) {
    toast(String(e), true);
  }
}

async function restoreBackup() {
  if (!currentProfile.value || !selectedBackup.value) return;
  if (
    !confirm(
      `用「${selectedBackup.value}」恢复 ${currentProfile.value}？\n` +
        `此操作会用所选备份覆盖当前档案，无法撤销。\n请确认已不需要当前进度（如需保留请先手动备份）。`,
    )
  )
    return;
  try {
    await invoke("stop_watchers");
    const r = await invoke<{ cache_deleted: boolean }>("restore_backup", {
      profile: currentProfile.value,
      backupName: selectedBackup.value,
    });
    toast("恢复完成" + (r.cache_deleted ? "，已清理 remotecache.vdf" : ""));
    await invoke("start_watchers");
    refreshBackups();
  } catch (e) {
    toast(String(e), true);
    try {
      await invoke("start_watchers");
    } catch (_) {}
  }
}

async function deleteBackup() {
  if (!currentProfile.value || !selectedBackup.value) return;
  if (!confirm(`确定删除备份「${selectedBackup.value}」吗？此操作不可恢复。`)) return;
  try {
    await invoke("delete_backup", {
      profile: currentProfile.value,
      backupName: selectedBackup.value,
    });
    selectedBackup.value = null;
    toast("已删除");
    refreshBackups();
  } catch (e) {
    toast(String(e), true);
  }
}

async function toggleWatch() {
  try {
    if (watching.value.length > 0) {
      await invoke("stop_watchers");
      toast("监控已停止（存档变化不再自动备份）");
    } else {
      const started = await invoke<string[]>("start_watchers");
      toast(`监控已启动：${started.join(", ") || "无新增档案"}`);
    }
    refreshStatus();
  } catch (e) {
    toast(String(e), true);
  }
}

/* ========== 事件 ========== */
let unlisteners: UnlistenFn[] = [];
async function setupEvents() {
  unlisteners.push(
    await listen<BackupEntry>("auto-backup-done", (ev) => {
      watchStatusOverride.value = `已自动备份：${ev.payload.name}`;
      watchStatusKind.value = "good";
      refreshBackups();
    }),
    await listen<string>("auto-backup-error", (ev) => {
      toast("自动备份失败：" + ev.payload, true);
    }),
    await listen("raid-start", () => {
      watchStatusOverride.value = "副本中（等待回城自动备份）";
      watchStatusKind.value = "warn";
    }),
    await listen("raid-end", () => {
      watchStatusOverride.value = "已回城，正在自动备份…";
      watchStatusKind.value = "good";
    }),
  );
}

onMounted(async () => {
  await setupEvents();
  await refreshStatus();
  await refreshProfiles();
  // 启动兜底：应用侧已自动启动监控，此处幂等确保生效
  try {
    await invoke("start_watchers");
  } catch (e) {
    console.error(e);
  }
  await refreshStatus();
  // 每 20 秒刷新状态与档案（自动接入新建档案）
  window.setInterval(() => {
    refreshStatus();
    refreshProfiles();
  }, 20000);
});

onUnmounted(() => {
  for (const un of unlisteners) un();
});
</script>

<template>
  <header>
    <div class="brand">
      <span class="brand-mark">▣</span>
      <div>
        <h1>暗黑地牢 存档管家</h1>
        <p class="sub">Darkest Dungeon Save Manager</p>
      </div>
    </div>
    <div class="status-wrap">
      <span
        id="game-status"
        class="pill"
        :class="gameRunning ? 'warn' : 'good'"
      >{{ gameRunning ? "⚠ 游戏运行中（恢复前需退出）" : "游戏未运行" }}</span>
      <span id="watch-status" class="pill" :class="watchStatusKind">{{ watchStatusText }}</span>
    </div>
  </header>

  <main>
    <ProfileSidebar
      :profiles="profiles"
      :current="currentProfile"
      :path-text="remotePath"
      @select="selectProfile"
    />

    <section id="detail">
      <div class="detail-head">
        <div class="detail-title">{{ currentProfile ?? "未选择档案" }}</div>
        <div class="detail-actions">
          <button class="btn btn-ghost" @click="toggleWatch">{{ watchBtnText }}</button>
          <button class="btn btn-primary" :disabled="!currentProfile" @click="manualBackup">＋ 手动备份</button>
          <button class="btn btn-danger" :disabled="!selectedBackup" @click="restoreBackup">恢复所选</button>
          <button class="btn btn-ghost" :disabled="!selectedBackup" @click="deleteBackup">删除所选</button>
        </div>
      </div>

      <BackupPanel
        :backups="backups"
        :selected="selectedBackup"
        @select-backup="selectBackup"
      />

      <div class="tip">
        <b>说明：</b>每周第一个（绿色）为自动存档（回城时触发，每周最多一个）；其余为手动存档。恢复会用所选备份覆盖当前档案且无法撤销，请先确认；恢复前需退出游戏。监控监听存档文件夹变化，与游戏是否运行无关，可随时手动启停。
      </div>
    </section>
  </main>

  <div id="toast" class="toast" :class="{ show: toastMsg, err: toastErr }">{{ toastMsg }}</div>
</template>
