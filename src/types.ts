export type ServiceStatus = 'running' | 'starting' | 'stopped' | 'failed' | 'exited' | 'crashed' | 'restarting';

export interface Service {
  id: string;
  project: string;
  name: string;
  cmd: string;
  repo_path?: string;
  port: number | null;
  status: ServiceStatus;
  uptime: number;
  pid?: number | null;
  pkg: string;
  cpu: number;
  mem: number;
  framework: string;
  managed?: boolean;
  _ws?: string;
}

export interface Workspace {
  id: string;
  name: string;
  desc: string;
  swatch: string;
  path: string;
  projects: string[];
  services: Service[];
  sessions: number;
  lastOpened: string;
}

export interface GitInfo {
  branch: string;
  clean: boolean;
  ahead: number;
  behind: number;
  changed: number;
  last: string;
}

export interface EnvVar {
  k: string;
  v: string;
}

export interface Script {
  name: string;
  cmd: string;
  hot?: boolean;
}

export interface Project {
  id: string;
  name: string;
  workspace: string;
  path: string;
  icon: string;
  framework: string;
  language: string;
  pkg: string;
  node: string;
  git: GitInfo;
  scripts: Script[];
  env: EnvVar[];
  ports: number[];
  deps: number;
  dev: number;
}

export interface ActivityItem {
  ts: string;
  project: string;
  label: string;
  kind: 'ok' | 'info' | 'warn' | 'error';
}

export interface Session {
  id: string;
  title: string;
  when: string;
  duration: number;
  ws: string;
  projects: number;
  services: number;
  badge?: string;
  started_at_ms: number;
  ended_at_ms: number | null;
  status: string;
}

export interface LogLine {
  ts: string;
  src: string;
  msg: string;
  kind: 'ok' | 'info' | 'warn' | 'error';
  run_id?: string;
  timestamp_ms?: number;
  stream?: string;
}

export interface Port {
  id: string;
  port: number;
  svc: string;
  host: string;
  status: ServiceStatus;
  ws: string;
  group: string;
}

export interface PortEdge {
  from: string;
  to: string;
}

export interface Repo {
  id: string;
  name: string;
  path: string;
  framework: string;
  package_manager: string;
  scripts: Script[];
  has_env: boolean;
  env_files: string[];
  language: string;
  has_readme: boolean;
  has_license: boolean;
  has_docker: boolean;
  has_devcontainer: boolean;
  dependencies: PackageEntry[];
  dev_dependencies: PackageEntry[];
  git: GitInfo | null;
  // live-derived
  is_running: boolean;
  running_port: number | null;
  cpu: number;
  mem: number;
}

export interface PackageEntry {
  name: string;
  version: string;
}

export interface StoredService {
  id: string;
  name: string;
  repo_path: string;
  script: string;
  cmd: string;
}

export interface StoredWorkspace {
  id: string;
  name: string;
  color: string;
  services: StoredService[];
}

export interface HubDataShape {
  workspaces: Workspace[];
  projects: Record<string, Repo>;
  activity: ActivityItem[];
  sessions: Session[];
  logSeeds: Record<string, { kind: LogLine['kind']; msg: string }[]>;
  ports: Port[];
  portEdges: PortEdge[];
}
