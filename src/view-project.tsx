import React from 'react';
import type { LogLine, Repo, Service, Workspace } from './types';
import { tauriApi, type EnvEntry } from './tauri-api';
import { Ic } from './icons';
import { StatusBadge } from './shared';

interface ProjectViewProps {
  project: Repo;
  workspaces: Workspace[];
  services: Service[];
  logs: LogLine[];
  onBack: () => void;
  onRunScript: (script: string, cmd: string) => void;
  onOpenEditor: () => void;
  onOpenLogs: () => void;
}

type Tab = 'overview' | 'scripts' | 'git' | 'ports' | 'logs' | 'env' | 'packages';

export function ProjectView({
  project,
  workspaces,
  services,
  logs,
  onBack,
  onRunScript,
  onOpenEditor,
  onOpenLogs,
}: ProjectViewProps) {
  const [tab, setTab] = React.useState<Tab>('overview');
  const [env, setEnv] = React.useState<EnvEntry[]>([]);
  const [envLoading, setEnvLoading] = React.useState(false);
  const projectServices = services.filter((service) => service.project === project.id || service.repo_path === project.path);
  const projectLogs = logs.filter((line) =>
    projectServices.some((service) => service.id === line.src) || line.src.startsWith(`adhoc::${project.path}::`)
  );
  const ports = Array.from(new Set([
    project.running_port,
    ...projectServices.map((service) => service.port),
  ].filter((port): port is number => port != null)));
  const workspace = workspaces.find((item) => item.services.some((service) => service.repo_path === project.path));

  React.useEffect(() => {
    if (tab !== 'env' || project.env_files.length === 0) return;
    setEnvLoading(true);
    tauriApi.readEnvFile(`${project.path}/${project.env_files[0]}`)
      .then((entries) => setEnv(entries ?? []))
      .catch(() => setEnv([]))
      .finally(() => setEnvLoading(false));
  }, [tab, project.path, project.env_files]);

  const tabs: Array<{ id: Tab; label: string; count?: number }> = [
    { id: 'overview', label: 'Overview' },
    { id: 'scripts', label: 'Scripts', count: project.scripts.length },
    { id: 'git', label: 'Git' },
    { id: 'ports', label: 'Ports', count: ports.length },
    { id: 'logs', label: 'Logs', count: projectLogs.length },
    { id: 'env', label: 'Environment', count: project.env_files.length },
    { id: 'packages', label: 'Packages', count: project.dependencies.length + project.dev_dependencies.length },
  ];

  return (
    <div className="view"><div className="view-inner">
      <button className="btn sm ghost" onClick={onBack} style={{ marginBottom: 14 }}>
        <Ic.Chevron size={10} style={{ transform: 'rotate(180deg)' }} /> Back to repos
      </button>

      <div className="proj-head">
        <div className="proj-icon">{project.name.slice(0, 2).toUpperCase()}</div>
        <div style={{ minWidth: 0 }}>
          <h1 className="proj-title">{project.name}</h1>
          <div className="proj-sub">
            <span title={project.path}><Ic.Folder size={11} /> {project.path}</span>
            <span className="sep">·</span>
            <span>{project.framework}</span>
            {project.git && (
              <>
                <span className="sep">·</span>
                <span><Ic.Branch size={11} /> {project.git.branch}</span>
                <span className="sep">·</span>
                <span style={{ color: project.git.clean ? 'var(--ok)' : 'var(--warn)' }}>
                  {project.git.clean ? 'clean' : `${project.git.changed} changes`}
                </span>
              </>
            )}
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          {ports[0] && (
            <button className="btn sm primary" onClick={() => tauriApi.openUrl(`http://localhost:${ports[0]}`)}>
              <Ic.Globe size={11} /> localhost:{ports[0]}
            </button>
          )}
          <button className="btn sm ghost" onClick={onOpenEditor}><Ic.External size={11} /> Open in editor</button>
        </div>
      </div>

      <div className="proj-tabs">
        {tabs.map((item) => (
          <button key={item.id} className={'proj-tab' + (tab === item.id ? ' active' : '')} onClick={() => setTab(item.id)}>
            {item.label}{item.count != null ? <span className="badge">{item.count}</span> : null}
          </button>
        ))}
      </div>

      {tab === 'overview' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1.4fr 1fr', gap: 16 }}>
          <Panel title="Project">
            <Key label="Path" value={project.path} mono />
            <Key label="Workspace" value={workspace?.name ?? 'Not assigned'} />
            <Key label="Framework" value={project.framework || 'Unknown'} />
            <Key label="Language" value={project.language || 'Unknown'} />
            <Key label="Package manager" value={project.package_manager || 'None detected'} />
            <Key label="Process" value={project.is_running ? `Running${project.running_port ? ` on :${project.running_port}` : ''}` : 'Not running'} />
          </Panel>
          <Panel title="Repository files">
            <Truth label="README" value={project.has_readme} />
            <Truth label="License" value={project.has_license} />
            <Truth label="Docker" value={project.has_docker} />
            <Truth label="Devcontainer" value={project.has_devcontainer} />
            <Truth label="Environment files" value={project.has_env} />
          </Panel>
        </div>
      )}

      {tab === 'scripts' && (
        <Panel title="Detected scripts">
          {project.scripts.length === 0 ? <Empty text="No runnable scripts were detected." /> : project.scripts.map((script) => (
            <div key={script.name} className="script-row">
              <span className="name">{script.name}</span>
              <span className="cmd">{script.cmd}</span>
              <button className="btn sm primary" onClick={() => onRunScript(script.name, script.cmd)}>
                <Ic.Play size={11} /> Run
              </button>
            </div>
          ))}
        </Panel>
      )}

      {tab === 'git' && (
        <Panel title="Working tree">
          {!project.git ? <Empty text="Git status is unavailable for this repository." /> : (
            <>
              <Key label="Branch" value={project.git.branch} mono />
              <Key label="State" value={project.git.clean ? 'Clean' : `${project.git.changed} changed files`} />
              <Key label="Upstream" value={`${project.git.ahead} ahead, ${project.git.behind} behind`} />
              <Key label="Latest commit" value={project.git.last || 'No commit information'} mono />
              <div style={{ padding: '12px 14px', color: 'var(--fg-4)', fontSize: 12 }}>
                Git operations are read-only in this milestone.
              </div>
            </>
          )}
        </Panel>
      )}

      {tab === 'ports' && (
        <Panel title="Detected ports">
          {ports.length === 0 ? <Empty text="No listening ports are associated with this project." /> : ports.map((port) => (
            <div key={port} className="script-row">
              <span className="name mono">:{port}</span>
              <span className="cmd">http://localhost:{port}</span>
              <button className="btn sm ghost" onClick={() => tauriApi.openUrl(`http://localhost:${port}`)}>
                <Ic.External size={11} /> Open
              </button>
            </div>
          ))}
        </Panel>
      )}

      {tab === 'logs' && (
        <Panel title="Run logs" action={<button className="btn sm ghost" onClick={onOpenLogs}>Open full log viewer</button>}>
          {projectLogs.length === 0 ? <Empty text="No persisted runs or live output for this project." /> : (
            <div className="logs-body" style={{ maxHeight: 520 }}>
              {projectLogs.slice(-500).map((line, index) => (
                <div key={`${line.run_id ?? line.src}-${index}`} className={'log-line ' + line.kind}>
                  <span className="ts">{line.ts}</span>
                  <span className="src">{line.src.split('::').pop()}</span>
                  <span className="msg">{line.msg}</span>
                </div>
              ))}
            </div>
          )}
        </Panel>
      )}

      {tab === 'env' && (
        <Panel title="Environment" action={<span className="mono" style={{ color: 'var(--fg-4)', fontSize: 11 }}>{project.env_files.join(', ')}</span>}>
          {envLoading ? <Empty text="Loading environment file..." /> :
            project.env_files.length === 0 ? <Empty text="No supported environment files were detected." /> :
            env.length === 0 ? <Empty text="The environment file is empty or unreadable." /> :
            env.map((entry) => <Key key={entry.key} label={entry.key} value={entry.value} mono />)}
          {project.env_files.length > 0 && (
            <div style={{ padding: '12px 14px', color: 'var(--fg-4)', fontSize: 12 }}>
              Read-only view. Secret-looking values are redacted by the backend.
            </div>
          )}
        </Panel>
      )}

      {tab === 'packages' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
          <PackageList title="Dependencies" items={project.dependencies} />
          <PackageList title="Development dependencies" items={project.dev_dependencies} />
        </div>
      )}
    </div></div>
  );
}

function Panel({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="panel">
      <div className="panel-head">
        <div className="panel-title"><span className="dot" /> {title}</div>
        {action}
      </div>
      {children}
    </div>
  );
}

function Key({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '160px 1fr', gap: 12, padding: '10px 14px', borderBottom: '1px solid var(--line-soft)', fontSize: 12 }}>
      <span style={{ color: 'var(--fg-4)' }}>{label}</span>
      <span className={mono ? 'mono' : ''} style={{ color: 'var(--fg-1)', overflowWrap: 'anywhere' }}>{value}</span>
    </div>
  );
}

function Truth({ label, value }: { label: string; value: boolean }) {
  return <Key label={label} value={value ? 'Detected' : 'Not detected'} />;
}

function Empty({ text }: { text: string }) {
  return <div style={{ padding: '28px 16px', textAlign: 'center', color: 'var(--fg-4)', fontSize: 12 }}>{text}</div>;
}

function PackageList({ title, items }: { title: string; items: Array<{ name: string; version: string }> }) {
  return (
    <Panel title={title}>
      {items.length === 0 ? <Empty text={`No ${title.toLowerCase()} detected.`} /> : items.map((item) => (
        <div key={item.name} className="script-row">
          <span className="name mono">{item.name}</span>
          <span className="cmd">{item.version}</span>
        </div>
      ))}
    </Panel>
  );
}
