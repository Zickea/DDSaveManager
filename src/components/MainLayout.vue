<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { NButton, NDrawer, NDrawerContent, NInputNumber, NLayout, NLayoutContent, NLayoutHeader, NLayoutSider, NModal, NSwitch, NTag, useDialog, useMessage } from "naive-ui";
import type { BackupEntry, ProfileInfo, Settings, StatusInfo } from "../types";
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

/* ========== 设置 ========== */
const showSettings = ref(false);
const settings = ref<Settings | null>(null);

async function loadSettings() {
  try {
    settings.value = await invoke<Settings>("get_settings");
  } catch (e) {
    console.error("get_settings:", e);
  }
}

async function saveSettings(next: Settings) {
  settings.value = next;
  try {
    await invoke("set_settings", { settings: next });
  } catch (e) {
    message.error(String(e));
  }
}

// 以当前设置为底合并局部修改（保证类型完整）
function patchSettings(patch: Partial<Settings>): Settings {
  return { auto_start: false, keep_auto_weeks: 0, confirm_rollback: true, ...settings.value, ...patch };
}

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
  await loadSettings();
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
      <div class="header-right">
        <n-button size="small" quaternary @click="showSettings = true">设置</n-button>
        <div class="status-wrap">
          <n-tag :type="gameRunning ? 'warning' : 'success'" size="small" :bordered="false">
            {{ gameRunning ? "⚠ 游戏运行中（恢复前需退出）" : "游戏未运行" }}
          </n-tag>
          <n-tag :type="watchStatusKind" size="small" :bordered="false">
            {{ watchStatusText }}
          </n-tag>
        </div>
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

    <n-drawer v-model:show="showSettings" placement="right" :width="340">
      <n-drawer-content title="设置" closable>
        <div class="settings-list">
          <div class="setting-row">
            <div class="setting-label">
              <div class="setting-name">开机自启</div>
              <div class="setting-desc">Windows 登录后自动启动并监控存档</div>
            </div>
            <n-switch
              :value="settings?.auto_start"
              @update:value="(v) => saveSettings(patchSettings({ auto_start: v }))"
            />
          </div>
          <div class="setting-row">
            <div class="setting-label">
              <div class="setting-name">自动备份保留</div>
              <div class="setting-desc">只保留最近 N 周的自动档（0 = 全部保留；手动档始终保留）</div>
            </div>
            <n-input-number
              :value="settings?.keep_auto_weeks ?? 0"
              :min="0"
              :max="999"
              size="small"
              style="width: 96px"
              @update:value="(v) => saveSettings(patchSettings({ keep_auto_weeks: v ?? 0 }))"
            />
          </div>
          <div class="setting-row">
            <div class="setting-label">
              <div class="setting-name">恢复前回退提示</div>
              <div class="setting-desc">恢复旧周存档时，提示将回退多少周</div>
            </div>
            <n-switch
              :value="settings?.confirm_rollback"
              @update:value="(v) => saveSettings(patchSettings({ confirm_rollback: v }))"
            />
          </div>
        </div>
      </n-drawer-content>
    </n-drawer>
  </n-layout>
</template>

<style scoped>
.header-right { display: flex; align-items: center; gap: 10px; }
.settings-list { display: flex; flex-direction: column; gap: 20px; }
.setting-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
.setting-label { flex: 1; min-width: 0; }
.setting-name { font-size: 13px; color: var(--text); }
.setting-desc { font-size: 11px; color: var(--text-faint); margin-top: 3px; line-height: 1.5; }
</style>
