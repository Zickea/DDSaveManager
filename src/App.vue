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
