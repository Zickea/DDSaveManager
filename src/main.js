// 暗黑地牢 存档管家 前端逻辑（Tauri 2 全局 API）
const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

let profiles = [];
let currentProfile = null;
let selectedBackup = null;
const knownProfiles = new Set();

const $ = (id) => document.getElementById(id);

/* ========== Toast ========== */
let toastTimer = null;
function toast(msg, isErr = false) {
  const el = $("toast");
  el.textContent = msg;
  el.className = "toast show" + (isErr ? " err" : "");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { el.className = "toast"; }, 3500);
}

/* ========== 状态 ========== */
async function refreshStatus() {
  try {
    const s = await invoke("get_status");
    $("remote-path").textContent =
      "存档目录：" + (s.remote_dir || "未找到（请确认已安装并运行过游戏）") +
      "\n备份保存在各 profile_N 目录内（与官方 backup 同级）";
    const gs = $("game-status");
    if (s.game_running) {
      gs.textContent = "⚠ 游戏运行中（恢复前需退出）";
      gs.className = "pill warn";
    } else {
      gs.textContent = "游戏未运行";
      gs.className = "pill good";
    }
    const ws = $("watch-status");
    if (s.watching.length > 0) {
      ws.textContent = `监控中：${s.watching.length} 个档案`;
      ws.className = "pill good";
      $("btn-watch").textContent = "停止监控";
      $("btn-watch").disabled = false;
    } else {
      ws.textContent = "监控未启动";
      ws.className = "pill off";
      $("btn-watch").textContent = "启动监控";
      $("btn-watch").disabled = false;
    }
  } catch (e) {
    console.error(e);
  }
}

/* ========== 档案 ========== */
async function refreshProfiles() {
  profiles = await invoke("get_profiles");
  const list = $("profile-list");
  if (profiles.length === 0) {
    list.innerHTML = '<div class="empty">未找到任何档案（profile_N），请先进入游戏创建战役</div>';
    return;
  }
  // 自动发现新档案并接入监控（幂等）
  const fresh = profiles.filter((p) => !knownProfiles.has(p.name));
  for (const p of profiles) knownProfiles.add(p.name);
  if (fresh.length > 0) {
    try {
      await invoke("start_watchers");
      refreshStatus();
    } catch (e) {
      console.error("start_watchers:", e);
    }
  }
  list.innerHTML = "";
  for (const p of profiles) {
    const card = document.createElement("div");
    card.className = "profile-card" + (p.name === currentProfile ? " active" : "");
    card.innerHTML =
      `<div class="profile-name">${p.name}</div>` +
      `<div class="profile-week">当前：${p.week != null ? "第 " + p.week + " 周" : "未知"}</div>`;
    card.onclick = () => selectProfile(p.name);
    list.appendChild(card);
  }
  if (!currentProfile && profiles.length > 0) {
    selectProfile(profiles[0].name);
  }
}

function selectProfile(name) {
  currentProfile = name;
  selectedBackup = null;
  $("detail-title").textContent = name;
  $("btn-restore").disabled = true;
  $("btn-delete").disabled = true;
  refreshProfiles(); // 刷新高亮
  refreshBackups();
}

/* ========== 备份列表 ========== */
async function refreshBackups() {
  if (!currentProfile) return;
  const backups = await invoke("list_backups", { profile: currentProfile });
  const list = $("backup-list");

  if (backups.length === 0) {
    list.innerHTML = '<div class="empty">该档案还没有备份。点击「＋ 手动备份」，或启动监控等待回城自动备份</div>';
    return;
  }

  // 按周分组（week 降序，未知周放最后）；组内：auto 第一，其余按时间（名称字典序）降序
  const groups = new Map();
  for (const b of backups) {
    const key = b.week != null ? b.week : -1;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(b);
  }
  const weeks = [...groups.keys()].sort((a, b) => b - a);
  for (const w of weeks) {
    groups.get(w).sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "auto" ? -1 : 1;
      return b.name.localeCompare(a.name);
    });
  }

  list.innerHTML = "";
  for (const w of weeks) {
    const group = document.createElement("div");
    group.className = "week-group";

    const header = document.createElement("div");
    header.className = "week-header";
    header.textContent =
      w === -1 ? "未知周数" : w === 0 ? "第 0 周（教学关）" : `第 ${w} 周`;
    group.appendChild(header);

    for (const b of groups.get(w)) {
      const row = document.createElement("div");
      row.className = "backup-row" + (b.name === selectedBackup ? " selected" : "");
      row.innerHTML =
        `<span class="badge badge-${b.kind}">${b.kind === "auto" ? "自动" : "手动"}</span>` +
        `<span class="backup-time">${b.timestamp.replace("_", " ")}</span>`;
      row.onclick = () => {
        selectedBackup = b.name;
        $("btn-restore").disabled = false;
        $("btn-delete").disabled = false;
        refreshBackups();
      };
      group.appendChild(row);
    }
    list.appendChild(group);
  }
}

/* ========== 动作 ========== */
async function manualBackup() {
  if (!currentProfile) return;
  try {
    const entry = await invoke("manual_backup", { profile: currentProfile });
    toast(`已手动备份：${entry.name}`);
    refreshBackups();
  } catch (e) {
    toast(String(e), true);
  }
}

async function restoreBackup() {
  if (!currentProfile || !selectedBackup) return;
  if (!confirm(
    `用「${selectedBackup}」恢复 ${currentProfile}？\n` +
    `此操作会用所选备份覆盖当前档案，无法撤销。\n请确认已不需要当前进度（如需保留请先手动备份）。`
  )) return;
  try {
    // 暂停监控，避免恢复过程的大量文件写入触发误判；恢复后重新启动
    await invoke("stop_watchers");
    const r = await invoke("restore_backup", {
      profile: currentProfile,
      backupName: selectedBackup,
    });
    toast("恢复完成" + (r.cache_deleted ? "，已清理 remotecache.vdf" : ""));
    await invoke("start_watchers");
    refreshBackups();
  } catch (e) {
    toast(String(e), true);
    try { await invoke("start_watchers"); } catch (_) {}
  }
}

async function deleteBackup() {
  if (!currentProfile || !selectedBackup) return;
  if (!confirm(`确定删除备份「${selectedBackup}」吗？此操作不可恢复。`)) return;
  try {
    await invoke("delete_backup", { profile: currentProfile, backupName: selectedBackup });
    selectedBackup = null;
    $("btn-restore").disabled = true;
    $("btn-delete").disabled = true;
    toast("已删除");
    refreshBackups();
  } catch (e) {
    toast(String(e), true);
  }
}

async function toggleWatch() {
  try {
    const s = await invoke("get_status");
    if (s.watching.length > 0) {
      await invoke("stop_watchers");
      toast("监控已停止（存档变化不再自动备份）");
    } else {
      const started = await invoke("start_watchers");
      toast(`监控已启动：${started.join(", ") || "无新增档案"}`);
    }
    refreshStatus();
  } catch (e) {
    toast(String(e), true);
  }
}

/* ========== 事件 ========== */
async function setupEvents() {
  await listen("auto-backup-done", (ev) => {
    // 静默提示（不弹 toast，避免连续自动备份时的弹窗闪烁）
    const ws = $("watch-status");
    ws.textContent = `已自动备份：${ev.payload.name}`;
    ws.className = "pill good";
    refreshBackups();
  });
  await listen("auto-backup-error", (ev) => {
    toast("自动备份失败：" + ev.payload, true);
  });
  await listen("raid-start", () => {
    $("watch-status").textContent = "副本中（等待回城自动备份）";
    $("watch-status").className = "pill warn";
  });
  await listen("raid-end", () => {
    $("watch-status").textContent = "已回城，正在自动备份…";
    $("watch-status").className = "pill good";
  });
}

/* ========== 启动 ========== */
$("btn-manual").onclick = manualBackup;
$("btn-restore").onclick = restoreBackup;
$("btn-delete").onclick = deleteBackup;
$("btn-watch").onclick = toggleWatch;

(async () => {
  await setupEvents();
  await refreshStatus();
  await refreshProfiles();
  // 启动兜底：应用侧已自动启动监控，此处幂等确保生效
  try { await invoke("start_watchers"); } catch (e) { console.error(e); }
  await refreshStatus();
  // 每 20 秒刷新状态与档案（自动接入新建档案）
  setInterval(async () => {
    await refreshStatus();
    await refreshProfiles();
  }, 20000);
})();
