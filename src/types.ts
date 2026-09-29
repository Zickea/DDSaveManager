// 与后端 Rust 命令返回结构对应的类型定义
export interface ProfileInfo {
  name: string;
  week: number | null;
}

export interface StatusInfo {
  remote_dir: string | null;
  game_running: boolean;
  watching: string[];
}

export interface BackupEntry {
  name: string;
  timestamp: string;
  week: number | null;
  kind: string; // auto / manual
}

export interface RestoreResult {
  cache_deleted: boolean;
}

export interface Settings {
  auto_start: boolean;
  keep_auto_weeks: number;
  confirm_rollback: boolean;
}
