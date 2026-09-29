import { describe, expect, it, vi } from 'vitest';
import { buildRepos, mapHistorySessions } from './App';
import type { HistorySession, LivePort, ProcessInfo, WorkspaceGroup } from './tauri-api';

describe('live project model', () => {
  it('combines scanner metadata with matching process and port data', () => {
    const groups: WorkspaceGroup[] = [{
      id: 'demo',
      name: 'Demo',
      path: '/code/demo',
      projects: [{
        path: '/code/demo',
        name: 'demo',
        framework: 'Vite + React',
        package_manager: 'npm',
        scripts: [{ name: 'dev', cmd: 'vite' }],
        has_git: true,
        has_env: true,
        env_files: ['.env'],
        language: 'TypeScript',
        has_readme: true,
        has_license: false,
        has_docker: true,
        has_devcontainer: false,
        dependencies: [{ name: 'react', version: '^18' }],
        dev_dependencies: [{ name: 'vite', version: '^5' }],
        git: {
          branch: 'main',
          ahead: 1,
          behind: 0,
          changed: 2,
          staged: 0,
          untracked: 1,
          clean: false,
          last_commit_message: 'test',
          last_commit_hash: '12345678',
        },
      }],
    }];
    const processes: ProcessInfo[] = [{
      pid: 42,
      name: 'node',
      cmd: ['node', 'vite'],
      cwd: '/code/demo',
      cpu_usage: 3.5,
      memory_kb: 131072,
      status: 'Run',
    }];
    const ports: LivePort[] = [{ port: 5173, pid: 42, process_name: 'node', protocol: 'tcp' }];

    const [repo] = buildRepos(groups, processes, ports);

    expect(repo.id).toBe('repo::/code/demo');
    expect(repo.running_port).toBe(5173);
    expect(repo.mem).toBe(128);
    expect(repo.git?.last).toContain('12345678');
    expect(repo.dependencies).toEqual([{ name: 'react', version: '^18' }]);
  });
});

describe('history session mapping', () => {
  it('maps active persisted sessions without fabricated fields', () => {
    vi.spyOn(Date, 'now').mockReturnValue(15_000);
    const sessions: HistorySession[] = [{
      id: 'session-1',
      workspace_id: 'ws-1',
      workspace_name: 'Workspace',
      title: 'Workspace',
      started_at_ms: 5_000,
      ended_at_ms: null,
      status: 'active',
      service_count: 2,
    }];

    const [session] = mapHistorySessions(sessions);

    expect(session.duration).toBe(10);
    expect(session.badge).toBe('ACTIVE');
    expect(session.services).toBe(2);
    expect(session.ws).toBe('ws-1');
    vi.restoreAllMocks();
  });
});
