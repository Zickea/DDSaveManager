<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { NButton, NLayout, NLayoutContent, NLayoutHeader, NLayoutSider, NModal, NTag, useDialog, useMessage } from "naive-ui";
import type { BackupEntry, ProfileInfo, StatusInfo } from "../types";
import ProfileSidebar from "./ProfileSidebar.vue";
import BackupPanel from "./BackupPanel.vue";

const message = useMessage();
const dialog = useDialog();

const profiles = ref<ProfileInfo[]>([]);
const currentProfile = ref<string | null>(null);
const selectedBackup = ref<string | null>(null);
const gameRunning = ref(false);
const watching = ref<string[]>([]);
const remotePath = ref("");

// 监控状态：事件可临时覆盖显示文本，下一次状态刷新自动还原
const watchStatusOverride = ref<string | null>(null);
const watchStatusKind = ref<"success" | "warning" | "default">("default");
const watchStatusText = computed(() =>
  watchStatusOverride.value ??
  (watching.value.length > 0 ? `监控中：${watching.value.length} 个档案` : "监控未启动"),
);
const watchBtnText = computed(() => (watching.value.length > 0 ? "停止监控" : "启动监控"));

const backups = ref<BackupEntry[]>([]);

/* ========== 状态 ========== */
async function refreshStatus() {
  try {
    const s: StatusInfo = await invoke("get_status");
    remotePath.value = s.remote_dir
      ? `存档目录：${s.remote_dir}\n备份文件夹 DDSL_save 保存在各 profile_N 目录内`
      : "存档目录：未找到（请确认已安装并运行过游戏）";
    gameRunning.value = s.game_running;
    watching.value = s.watching;
    watchStatusOverride.value = null;
    watchStatusKind.value = watching.value.length > 0 ? "success" : "default";
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
async function launchGame() {
  try {
    await invoke("launch_game");
    message.success("已请求启动《暗黑地牢》（Steam 拉起中）");
  } catch (e) {
    message.error(String(e));
  }
}

async function manualBackup() {
  if (!currentProfile.value) return;
  try {
    const entry = await invoke<BackupEntry>("manual_backup", {
      profile: currentProfile.value,
    });
    message.success(`已手动备份：${entry.name}`);
    refreshBackups();
  } catch (e) {
    message.error(String(e));
  }
}

function restoreBackup() {
  if (!currentProfile.value || !selectedBackup.value) return;
  dialog.warning({
    title: "恢复存档",
    content: `用「${selectedBackup.value}」恢复 ${currentProfile.value}？\n此操作会用所选备份覆盖当前档案，无法撤销。请确认已不需要当前进度（如需保留请先手动备份）。`,
    positiveText: "确认恢复",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await invoke("stop_watchers");
        const r = await invoke<{ cache_deleted: boolean }>("restore_backup", {
          profile: currentProfile.value,
          backupName: selectedBackup.value,
        });
        message.success("恢复完成" + (r.cache_deleted ? "，已清理 remotecache.vdf" : ""));
        await invoke("start_watchers");
        refreshBackups();
      } catch (e) {
        message.error(String(e));
        try {
          await invoke("start_watchers");
        } catch (_) {}
      }
    },
  });
}

function deleteBackup() {
  if (!currentProfile.value || !selectedBackup.value) return;
  dialog.warning({
    title: "删除备份",
    content: `确定删除备份「${selectedBackup.value}」吗？此操作不可恢复。`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await invoke("delete_backup", {
          profile: currentProfile.value,
          backupName: selectedBackup.value,
        });
        selectedBackup.value = null;
        message.success("已删除");
        refreshBackups();
      } catch (e) {
        message.error(String(e));
      }
    },
  });
}

async function toggleWatch() {
  try {
    if (watching.value.length > 0) {
      await invoke("stop_watchers");
      message.info("监控已停止（存档变化不再自动备份）");
    } else {
      const started = await invoke<string[]>("start_watchers");
      message.info(`监控已启动：${started.join(", ") || "无新增档案"}`);
    }
    refreshStatus();
  } catch (e) {
    message.error(String(e));
  }
}

/* ========== 删除档案 ========== */
const showDeleteModal = ref(false);
const deleteTarget = ref<string | null>(null);

function askDeleteProfile(name: string) {
  deleteTarget.value = name;
  showDeleteModal.value = true;
}

async function clearBackupsOnly() {
  const name = deleteTarget.value;
  if (!name) return;
  try {
    await invoke("clear_profile_backups", { profile: name });
    message.success(`已删除 ${name} 的全部备份（游戏进度保留）`);
    showDeleteModal.value = false;
    afterProfileChanged(name);
  } catch (e) {
    message.error(String(e));
  }
}

function deleteProfileAll() {
  const name = deleteTarget.value;
  if (!name) return;
  dialog.error({
    title: "删除整个档案",
    content: `确定要删除档案 ${name} 吗？\n这将删除该存档位的全部游戏进度与所有备份（含官方 backup 目录），不可恢复！\n请确认游戏已退出。`,
    positiveText: "删除档案",
    negativeText: "取消",
    onPositiveClick: async () => {
      try {
        await invoke("stop_watchers");
        await invoke("delete_profile", { profile: name });
        message.success(`已删除档案 ${name}`);
        showDeleteModal.value = false;
        afterProfileChanged(name);
        await invoke("start_watchers");
      } catch (e) {
        message.error(String(e));
        try {
          await invoke("start_watchers");
        } catch (_) {}
      }
    },
  });
}

// 删除后收尾：若删的是当前档案则清空选中；从 knownProfiles 移除以便重建后自动接入监控
function afterProfileChanged(name: string) {
  if (currentProfile.value === name) {
    currentProfile.value = null;
    selectedBackup.value = null;
    backups.value = [];
  }
  knownProfiles.delete(name);
  refreshProfiles();
  refreshStatus();
}

/* ========== 事件 ========== */
let unlisteners: UnlistenFn[] = [];
async function setupEvents() {
  unlisteners.push(
    await listen<BackupEntry>("auto-backup-done", (ev) => {
      watchStatusOverride.value = `已自动备份：${ev.payload.name}`;
      watchStatusKind.value = "success";
      refreshBackups();
    }),
    await listen<string>("auto-backup-error", (ev) => {
      message.error("自动备份失败：" + ev.payload);
    }),
    await listen("raid-start", () => {
      watchStatusOverride.value = "副本中（等待回城自动备份）";
      watchStatusKind.value = "warning";
    }),
    await listen("raid-end", () => {
      watchStatusOverride.value = "已回城，正在自动备份…";
      watchStatusKind.value = "success";
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
  <n-layout class="app-root">
    <n-layout-header class="app-header" bordered>
      <div class="brand">
        <span class="brand-mark">▣</span>
        <div>
          <h1>暗黑地牢 存档管家</h1>
          <p class="sub">Darkest Dungeon Save Manager</p>
        </div>
        <n-button size="small" :disabled="gameRunning" @click="launchGame">
          运行《暗黑地牢》
        </n-button>
      </div>
      <div class="status-wrap">
        <n-tag :type="gameRunning ? 'warning' : 'success'" size="small" :bordered="false">
          {{ gameRunning ? "⚠ 游戏运行中（恢复前需退出）" : "游戏未运行" }}
        </n-tag>
        <n-tag :type="watchStatusKind" size="small" :bordered="false">
          {{ watchStatusText }}
        </n-tag>
      </div>
    </n-layout-header>

    <n-layout class="app-body" has-sider>
      <n-layout-sider bordered :width="250" style="height: 100%;" :native-scrollbar="false">
        <ProfileSidebar
          :profiles="profiles"
          :current="currentProfile"
          :path-text="remotePath"
          @select="selectProfile"
          @delete-profile="askDeleteProfile"
        />
      </n-layout-sider>

      <n-layout-content content-class="app-content">
        <div class="detail-head">
          <div class="detail-title">{{ currentProfile ?? "未选择档案" }}</div>
          <div class="detail-actions">
            <n-button size="small" @click="toggleWatch">{{ watchBtnText }}</n-button>
            <n-button size="small" type="primary" :disabled="!currentProfile" @click="manualBackup">
              ＋ 手动备份
            </n-button>
            <n-button size="small" type="error" :disabled="!selectedBackup" @click="restoreBackup">
              恢复所选
            </n-button>
            <n-button size="small" :disabled="!selectedBackup" @click="deleteBackup">
              删除所选
            </n-button>
          </div>
        </div>

        <BackupPanel :backups="backups" :selected="selectedBackup" @select-backup="selectBackup" />

        <div class="tip">
          <b>说明：</b>每周第一个（绿色）为自动存档（回城时触发，每周最多一个）；其余为手动存档。恢复会用所选备份覆盖当前档案且无法撤销，请先确认；恢复前需退出游戏。监控监听存档文件夹变化，与游戏是否运行无关，可随时手动启停。侧栏档案右侧 ✕ 可删除（备份或整个档案）。
        </div>
      </n-layout-content>
    </n-layout>

    <n-modal v-model:show="showDeleteModal">
      <div class="delete-modal">
        <h3>删除档案 {{ deleteTarget }}</h3>
        <p class="dim">请选择删除范围（均为不可恢复操作，请谨慎）：</p>
        <div class="del-actions">
          <n-button type="warning" block @click="clearBackupsOnly">
            仅删除全部备份（保留游戏进度）
          </n-button>
          <n-button type="error" block @click="deleteProfileAll">
            删除整个档案（进度 + 备份，不可恢复）
          </n-button>
        </div>
      </div>
    </n-modal>
  </n-layout>
</template>
