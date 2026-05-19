/**
 * Thin wrapper around Tauri invoke calls with browser fallbacks.
 * Import and call these instead of directly using @tauri-apps/api.
 * In a browser (vite dev without tauri), all calls return empty/null
 * so the app works in both environments.
 */

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let _invoke: InvokeFn | null = null;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return Promise.resolve(null as T);
  if (!_invoke) {
    const mod = await import("@tauri-apps/api/core");
    _invoke = mod.invoke as InvokeFn;
  }
  return _invoke<T>(cmd, args);
}

// ── Types mirroring Rust structs ──────────────────────────────────────────────

export interface LivePort {
  port: number;
  pid: number | null;
  process_name: string | null;
  protocol: string;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  cmd: string[];
  cwd: string | null;
  cpu_usage: number;
  memory_kb: number;
  status: string;
}

export interface SystemStats {
  cpu_usage: number;
  memory_used_mb: number;
  memory_total_mb: number;
  load_avg: [number, number, number];
}

export interface GitStatus {
  branch: string;
  ahead: number;
  behind: number;
  changed: number;
  staged: number;
  untracked: number;
  clean: boolean;
  last_commit_message: string | null;
  last_commit_hash: string | null;
}

export interface DetectedProject {
  path: string;
  name: string;
  framework: string;
  package_manager: string;
  scripts: Array<{ name: string; cmd: string }>;
  has_git: boolean;
  has_env: boolean;
  env_files: string[];
}

export interface WorkspaceGroup {
  id: string;
  name: string;
  path: string;
  projects: DetectedProject[];
}

export interface EnvEntry {
  key: string;
  value: string;
  redacted: boolean;
}

// ── Commands ──────────────────────────────────────────────────────────────────

export const tauriApi = {
  scanPorts: () => invoke<LivePort[]>("scan_ports"),

  getProcesses: () => invoke<ProcessInfo[]>("get_processes"),

  killProcess: (pid: number) => invoke<void>("kill_process", { pid }),

  getSystemStats: () => invoke<SystemStats>("get_system_stats"),

  getGitStatus: (path: string) => invoke<GitStatus | null>("get_git_status", { path }),

  scanWorkspaces: (root: string, maxDepth?: number) =>
    invoke<DetectedProject[]>("scan_workspaces", { root, maxDepth }),

  scanWorkspaceGroups: (roots: string[]) =>
    invoke<WorkspaceGroup[]>("scan_workspace_groups", { roots }),

  findDefaultWorkspaceRoots: () =>
    invoke<string[]>("find_default_workspace_roots"),

  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),

  openUrl: (url: string) => invoke<void>("open_url", { url }),

  readEnvFile: (path: string) => invoke<EnvEntry[]>("read_env_file", { path }),
};
