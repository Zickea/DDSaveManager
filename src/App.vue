<script setup lang="ts">
import { darkTheme, dateZhCN, zhCN, NConfigProvider, NDialogProvider, NMessageProvider, type GlobalThemeOverrides } from "naive-ui";
import MainLayout from "./components/MainLayout.vue";

// 将 Naive 主题覆盖为暗黑地牢风格（深棕底、金色强调、血红警示、暗绿主操作）
const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#5a7d5a",
    primaryColorHover: "#6b906b",
    primaryColorPressed: "#4a6a4a",
    primaryColorSuppl: "#5a7d5a",
    errorColor: "#a33b2e",
    errorColorHover: "#c04a3a",
    errorColorPressed: "#8a2e22",
    warningColor: "#c9a86a",
    infoColor: "#c9a86a",
    successColor: "#8fc08f",
    borderRadius: "8px",
    bodyColor: "#171310",
    cardColor: "#241d17",
    modalColor: "#241d17",
    popoverColor: "#241d17",
    textColorBase: "#e8dcc8",
    textColor1: "#e8dcc8",
    textColor2: "#b8a98c",
    textColor3: "#8a7a5e",
    borderColor: "#3a2e20",
    dividerColor: "#3a2e20",
    hoverColor: "rgba(201,168,106,0.08)",
  },
  Button: {
    textColor: "#e8dcc8",
    border: "1px solid #4a3b2a",
    borderRadiusMedium: "8px",
  },
  Tag: {
    borderRadius: "4px",
  },
};
</script>

<template>
  <n-config-provider
    :theme="darkTheme"
    :theme-overrides="themeOverrides"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <n-message-provider>
      <n-dialog-provider>
        <MainLayout />
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style>
:root {
  --bg: #171310;
  --bg-2: #241d17;
  --bg-3: #1d1813;
  --line: #3a2e20;
  --line-2: #4a3b2a;
  --text: #e8dcc8;
  --text-dim: #b8a98c;
  --text-faint: #8a7a5e;
  --gold: #c9a86a;
  --red: #a33b2e;
  --red-bright: #d07a6a;
  --green: #5a7d5a;
  --green-bright: #8fc08f;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

html, body, #app { height: 100%; }

body {
  background: var(--bg);
  color: var(--text);
  font-family: "Segoe UI", "Microsoft YaHei", system-ui, sans-serif;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

#app { display: flex; flex-direction: column; }

/* ===== 顶栏 ===== */
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  border-bottom: 1px solid var(--line);
  background: var(--bg-3);
  flex-shrink: 0;
}

.brand { display: flex; align-items: center; gap: 12px; }
.brand-mark {
  font-size: 26px;
  color: var(--red);
  border: 1px solid var(--red);
  width: 40px; height: 40px;
  display: flex; align-items: center; justify-content: center;
  border-radius: 8px;
  background: var(--bg-2);
}
h1 { font-size: 17px; font-weight: 700; letter-spacing: 1px; }
.sub { font-size: 11px; color: var(--text-faint); }

.status-wrap { display: flex; gap: 8px; }

/* ===== 主区 ===== */
main {
  flex: 1;
  display: flex;
  min-height: 0;
}

/* 侧栏 */
#sidebar {
  width: 250px;
  border-right: 1px solid var(--line);
  padding: 16px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--bg-3);
  overflow-y: auto;
}
.side-title { font-size: 12px; color: var(--gold); font-weight: 600; }
.profile-list { display: flex; flex-direction: column; gap: 8px; }
.profile-card {
  background: var(--bg-2);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 10px 12px;
  cursor: pointer;
  transition: border-color .15s;
  display: flex;
  align-items: center;
  gap: 8px;
}
.profile-card:hover { border-color: var(--line-2); }
.profile-card.active {
  border-color: var(--gold);
  border-left: 3px solid var(--gold);
  padding-left: 10px;
}
.profile-info { flex: 1; min-width: 0; }
.profile-name { font-size: 14px; font-weight: 700; }
.profile-week { font-size: 12px; color: var(--text-dim); margin-top: 3px; }
.del-btn { opacity: 0; transition: opacity .15s; flex-shrink: 0; }
.profile-card:hover .del-btn { opacity: 1; }
.side-path {
  font-size: 10px;
  color: var(--text-faint);
  word-break: break-all;
  line-height: 1.6;
  margin-top: auto;
}
.empty { color: var(--text-faint); font-size: 13px; padding: 8px 0; }

/* 详情区 */
#detail {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.detail-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid var(--line);
  gap: 12px;
  flex-wrap: wrap;
  flex-shrink: 0;
}
.detail-title { font-size: 18px; font-weight: 700; }
.detail-actions { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }

/* 备份列表 */
.backup-list {
  flex: 1;
  overflow-y: auto;
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.week-group { border-radius: 8px; overflow: hidden; border: 1px solid var(--line); }
.week-header {
  background: #3a2a1a;
  color: var(--text);
  font-size: 13px;
  font-weight: 700;
  padding: 7px 12px;
  border-bottom: 1px solid var(--line);
}
.backup-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  background: var(--bg-2);
  border-bottom: 1px solid var(--line);
  cursor: pointer;
  font-size: 13px;
}
.backup-row:last-child { border-bottom: none; }
.backup-row:hover { background: #2c2318; }
.backup-row.selected { background: #3a2a1a; border-left: 3px solid var(--gold); padding-left: 9px; }
.backup-time { color: var(--text-dim); flex: 1; }

.tip {
  font-size: 12px;
  color: var(--text-faint);
  padding: 10px 18px;
  border-top: 1px solid var(--line);
  background: var(--bg-3);
  line-height: 1.7;
  flex-shrink: 0;
}
.tip b { color: var(--gold); }

/* 删除档案弹窗 */
.delete-modal {
  background: var(--bg-2);
  border: 1px solid var(--line-2);
  border-radius: 10px;
  padding: 20px;
  width: 420px;
  max-width: 90vw;
}
.delete-modal h3 { font-size: 16px; margin-bottom: 8px; color: var(--red-bright); }
.delete-modal .dim { font-size: 13px; color: var(--text-dim); margin-bottom: 16px; }
.delete-modal .del-actions { display: flex; flex-direction: column; gap: 10px; }
</style>
