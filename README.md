# 暗黑地牢 存档管家（Darkest Dungeon Save Manager）

为《暗黑地牢》（Darkest Dungeon，Steam 版）设计的存档管理与读档（Save/Load）工具。
纯本地运行，无任何网络上传。

> 这个游戏没有常规的"存档/读档"入口。本工具通过**每周进副本前自动备份** + **一键恢复**，
> 让你可以安全地读档重来（打输了、死人了、被气到了，都可以回到进副本前）。
> 工具直接读写 Steam 存档目录，恢复前会完整校验状态，避免坏档。

---

## ⚠️ 最重要的一个使用规则（务必先读）

**如果在副本中途退出游戏（Alt+F4 / 任务管理器强退），然后恢复了城镇存档，
必须先完全退出 Steam 客户端（右下角托盘图标 → 右键 → 退出），再重新启动游戏。**

否则游戏会报错无法加载：

```
Assert Failed: (len == file_info.m_FileSize)
StorageManager::TransferFile ... didn't read whole file?
```

原因：游戏通过 Steamworks 云存储读写存档，运行中的 Steam 客户端会持有"上次副本中途"的文件大小清单。
本地恢复成城镇档后大小不一致，Steamworks 校验失败。**完全退出 Steam 后客户端会重新扫描磁盘，
清单与城镇档对齐，即可正常加载。**（已实测验证。）

正常流程（不进副本就退出游戏）不需要这一步。

---

## 功能特性

- **每周自动备份**：副本结束回到城镇时，自动生成该周进副本前的存档（按周命名 `week01` / `week02` …）
- **手动备份**：随时手动存档，支持填写备注；同周多次备份按时间区分
- **按周分组**：备份以周为单位分组展示，每周第一条为自动档，其余为手动档，总体按时间降序
- **一键恢复**：恢复时先清空当前档案再铺回备份（保留官方 backup 与我们自己的备份目录），并清理 Steam 云缓存
- **多档案支持**：同时管理 9 个战役档案（profile_0 ~ profile_8），互不干扰
- **档案信息卡片**：显示庄园名、游戏模式（极暗/血月/暗黑/无光…）、当前英雄数、离队数
- **副本状态拦截**：副本（未结算）状态下禁止手动备份；恢复副本残留档案前会预检并提示重启 Steam
- **恢复前回退提示**：显示将回退几周 / 备份比当前新几周
- **自动备份保留策略**：可设置只保留最近 N 周的自动档，防止备份无限膨胀
- **开机自启**、**系统托盘常驻**（关闭窗口后台运行，右键退出）、**单实例运行**
- **一键启动游戏**：标题栏「运行《暗黑地牢》」按钮，直接拉起游戏
- **打开存档目录**：一键在资源管理器中定位当前档案目录
- **暗黑地牢风格界面**：深棕 + 金色 + 血红的暗黑主题

## 工作原理

| 概念 | 说明 |
|---|---|
| 存档位置 | Steam 云目录 `...\Steam\userdata\<你的UID>\262060\remote\profile_N` |
| 备份存放 | 各 `profile_N\DDSL_save\` 子目录（与游戏官方 `backup` 文件夹同级） |
| 副本信号 | `persist.raid.json`：进副本时创建、回城时删除，工具监听它的出现/消失来判定状态 |
| 自动备份时机 | 副本结束回城（信号文件被删除）那一刻，即"第 N 周进副本前"的状态 |
| 周数语义 | 解析存档 `total_weeks` 字段后 **−1**（玩家视角：教学关 = 第 0 周，回城后 = 第 1 周） |
| 恢复安全性 | 游戏未运行时才允许恢复；副本残留恢复会预检并提示重启 Steam（见顶部警示） |

存档文件是 Red Hook 自研的 **dson** 二进制格式（后缀伪装成 `.json`），工具内部按实测校准的
字节布局直接解析周数、庄园名、模式、英雄数等元数据，不改动存档内容本身。

## 安装与使用

1. 从 [Releases](../../releases) 下载安装包（或便携版 `dd-save-manager.exe`）
2. 先运行过一次《暗黑地牢》（让 Steam 生成存档目录），然后启动本工具
3. 左侧选择战役档案，右侧查看备份；「手动备份」随时存档，「恢复」回到任意备份点
4. 打副本前会自动备份；打输/死人了就恢复，**若刚强退过副本请重启 Steam 再开游戏**

## 设置

| 设置项 | 默认 | 说明 |
|---|---|---|
| 开机自启 | 关 | 登录 Windows 时自动启动本工具 |
| 保留最近 N 周自动档 | 全部保留 | 超过 N 周的自动档自动清理（手动档不受影响） |
| 恢复前回退提示 | 开 | 恢复时显示将回退/前进的周数 |

设置保存在 `%APPDATA%\com.ddsl.savemanager\settings.json`。

## 从源码构建

```bash
# 前端依赖
npm install
# 开发模式
npm run tauri dev
# 打包（NSIS 安装包 + 便携版）
npm run tauri build
```

## 技术栈

- **后端**：Rust + Tauri 2（文件监控 notify、进程检测 tasklist、Win32 注册表）
- **前端**：Vite + Vue 3 + TypeScript + Naive UI
- **解析**：手写 dson 二进制读取器（周数 / 庄园名 / 模式 / 英雄数）

## 已知限制与说明

- 仅支持 **Windows** + Steam 版《暗黑地牢》
- 需要**先运行过一次游戏**（存档目录由 Steam 云生成）
- 本工具与游戏官方 `backup` 文件夹互不干扰；请勿在游戏运行时手动恢复（工具已拦截）
- 存档安全性：工具只读取/复制/替换存档文件，不修改存档内容；但**恢复操作不可撤销**，
  请在恢复前确认不需要当前进度（或先手动备份）

## 免责声明

本工具是独立开发的第三方工具，与 Red Hook Studios 无关。仅供个人存档管理使用。
使用本工具造成的一切存档损失由使用者自行承担（工具已尽可能校验与提示）。

## 许可证

[MIT](./LICENSE)

---

## English Summary

**Darkest Dungeon Save Manager** — a local save backup & restore (save-scum) tool for the
Steam version of *Darkest Dungeon*.

- Auto-backup at the start of each week (when you return to town after a quest)
- Manual backups with notes; grouped by week; newest first
- One-click restore with full state checking; manages all 9 campaign profiles
- **Important**: if you quit mid-quest and restore a town save, **fully quit the Steam
  client before launching the game** — otherwise Steamworks cloud-storage size checks fail
  (`StorageManager::TransferFile ... didn't read whole file?`) and the game won't load.
- Windows only. Local-only, no telemetry. Built with Rust + Tauri 2 + Vue 3.
