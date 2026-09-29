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
  language: string;
  has_readme: boolean;
  has_license: boolean;
  has_docker: boolean;
  has_devcontainer: boolean;
  dependencies: Array<{ name: string; version: string }>;
  dev_dependencies: Array<{ name: string; version: string }>;
  git: GitStatus | null;
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

export interface ServiceEvent {
  service_id: string;
  run_id: string;
  kind: "started" | "stdout" | "stderr" | "exited" | "error" | "stopped";
  message: string;
  pid: number | null;
  code: number | null;
  timestamp_ms: number;
  sequence: number;
  stream: string;
}

export interface ManagedServiceInfo {
  service_id: string;
  cwd: string;
  cmd: string;
  pid: number;
  started_at_ms: number;
  run_id: string;
}

export interface HistorySession {
  id: string;
  workspace_id: string;
  workspace_name: string;
  title: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  status: string;
  service_count: number;
}

export interface HistoryRun {
  id: string;
  session_id: string | null;
  service_id: string;
  project_id: string | null;
  workspace_id: string | null;
  name: string;
  cwd: string;
  command: string;
  pid: number | null;
  started_at_ms: number;
  ended_at_ms: number | null;
  status: string;
  exit_code: number | null;
}

export interface HistoryLog {
  id: number;
  run_id: string;
  service_id: string;
  timestamp_ms: number;
  sequence: number;
  stream: string;
  message: string;
}

export interface GitHubRepo {
  name: string;
  full_name: string;
  html_url: string;
  clone_url: string;
  ssh_url: string;
  private: boolean;
  description: string | null;
  default_branch: string;
  updated_at: string;
  language: string | null;
}

// ── Commands ──────────────────────────────────────────────────────────────────

export const tauriApi = {
  scanPorts: () => invoke<LivePort[]>("scan_ports"),

  getProcesses: () => invoke<ProcessInfo[]>("get_processes"),

  killProcess: (pid: number) => invoke<void>("kill_process", { pid }),

  startService: (
    serviceId: string,
    cwd: string,
    cmd: string,
    metadata: {
      name?: string;
      projectId?: string;
      workspaceId?: string;
      sessionId?: string;
    } = {},
  ) =>
    invoke<number>("start_service", { serviceId, cwd, cmd, ...metadata }),

  stopManagedService: (serviceId: string) =>
    invoke<void>("stop_service", { serviceId }),

  listManagedServices: () =>
    invoke<ManagedServiceInfo[]>("list_managed_services"),

  getSystemStats: () => invoke<SystemStats>("get_system_stats"),

  getGitStatus: (path: string) => invoke<GitStatus | null>("get_git_status", { path }),

  listGitHubRepos: () => invoke<GitHubRepo[]>("github_list_repos"),

  scanWorkspaces: (root: string, maxDepth?: number) =>
    invoke<DetectedProject[]>("scan_workspaces", { root, maxDepth }),

  scanWorkspaceGroups: (roots: string[]) =>
    invoke<WorkspaceGroup[]>("scan_workspace_groups", { roots }),

  findDefaultWorkspaceRoots: () =>
    invoke<string[]>("find_default_workspace_roots"),

  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),

  openUrl: (url: string) => invoke<void>("open_url", { url }),

  readEnvFile: (path: string) => invoke<EnvEntry[]>("read_env_file", { path }),

  createHistorySession: (workspaceId: string, workspaceName: string, title?: string) =>
    invoke<HistorySession>("create_history_session", { workspaceId, workspaceName, title }),

  listHistorySessions: (limit = 100) =>
    invoke<HistorySession[]>("list_history_sessions", { limit }),

  finalizeHistorySession: (sessionId: string, expectedRuns: number) =>
    invoke<void>("finalize_history_session", { sessionId, expectedRuns }),

  listHistoryRuns: (sessionId?: string, limit = 500) =>
    invoke<HistoryRun[]>("list_history_runs", { sessionId, limit }),

  listHistoryLogs: (runId?: string, query?: string, limit = 2000) =>
    invoke<HistoryLog[]>("list_history_logs", { runId, query, limit }),

  clearHistory: () => invoke<void>("clear_history"),
};

export async function listenToServiceEvents(handler: (event: ServiceEvent) => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const mod = await import("@tauri-apps/api/event");
  return mod.listen<ServiceEvent>("service://event", (event) => handler(event.payload));
}
