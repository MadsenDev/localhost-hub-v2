import React from 'react';
import type { Session, Workspace } from './types';
import { tauriApi, type HistoryRun } from './tauri-api';
import { Ic } from './icons';
import { formatDuration } from './utils';

interface SessionsViewProps {
  workspaces: Workspace[];
  sessions: Session[];
  onResume: (session: Session) => void;
  onOpenLogs: () => void;
}

export function SessionsView({ workspaces, sessions, onResume, onOpenLogs }: SessionsViewProps) {
  const [activeId, setActiveId] = React.useState(sessions[0]?.id ?? '');
  const [runs, setRuns] = React.useState<HistoryRun[]>([]);
  const [loading, setLoading] = React.useState(false);
  const active = sessions.find((session) => session.id === activeId) ?? sessions[0];

  React.useEffect(() => {
    if (!active) return;
    setLoading(true);
    tauriApi.listHistoryRuns(active.id)
      .then((items) => setRuns(items ?? []))
      .catch(() => setRuns([]))
      .finally(() => setLoading(false));
  }, [active?.id]);

  if (!active) {
    return (
      <div className="view"><div className="view-inner">
        <div className="eyebrow">History</div>
        <h1 className="h1">Sessions</h1>
        <div className="panel" style={{ marginTop: 18, padding: 40, textAlign: 'center', color: 'var(--fg-4)' }}>
          Workspace sessions appear after you use Boot all.
        </div>
      </div></div>
    );
  }

  const workspace = workspaces.find((item) => item.id === active.ws);

  return (
    <div className="view"><div className="view-inner">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end', marginBottom: 18 }}>
        <div>
          <div className="eyebrow" style={{ marginBottom: 4 }}>Persisted history</div>
          <h1 className="h1">Sessions</h1>
          <div style={{ color: 'var(--fg-3)', fontSize: 12.5, marginTop: 4 }}>
            Workspace boots and their service runs, stored locally.
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button className="btn sm ghost" onClick={onOpenLogs}><Ic.Logs size={11} /> Open logs</button>
          <button className="btn sm primary" onClick={() => onResume(active)} disabled={active.status === 'active'}>
            <Ic.Play size={11} /> Resume service set
          </button>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '300px 1fr', gap: 16 }}>
        <div className="panel" style={{ padding: 0, alignSelf: 'start' }}>
          <div className="panel-head"><div className="panel-title"><span className="dot" /> Session history</div></div>
          {sessions.map((session) => {
            const itemWorkspace = workspaces.find((item) => item.id === session.ws);
            return (
              <button
                key={session.id}
                className={'session-row' + (session.id === active.id ? ' active' : '')}
                onClick={() => setActiveId(session.id)}
                style={{ width: '100%', gridTemplateColumns: '10px 1fr', textAlign: 'left' }}
              >
                <span style={{ width: 8, height: 8, borderRadius: 2, background: itemWorkspace?.swatch ?? 'var(--fg-4)' }} />
                <span>
                  <span className="title">{session.title}</span>
                  <span className="meta" style={{ display: 'block', marginTop: 3 }}>
                    {session.when} · {formatDuration(session.duration)}
                  </span>
                </span>
              </button>
            );
          })}
        </div>

        <div>
          <div className="panel" style={{ marginBottom: 16 }}>
            <div className="panel-head">
              <div className="panel-title active"><span className="dot" /> {active.title}</div>
              <span className={'tag ' + (active.status === 'active' ? 'ok' : '')}>{active.status}</span>
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)' }}>
              <Metric label="Workspace" value={workspace?.name ?? active.ws} />
              <Metric label="Started" value={new Date(active.started_at_ms).toLocaleString()} />
              <Metric label="Duration" value={formatDuration(active.duration)} />
              <Metric label="Runs" value={String(active.services)} />
            </div>
          </div>

          <div className="panel">
            <div className="panel-head"><div className="panel-title"><span className="dot" /> Service runs</div></div>
            {loading ? (
              <div style={{ padding: 28, textAlign: 'center', color: 'var(--fg-4)' }}>Loading runs...</div>
            ) : runs.length === 0 ? (
              <div style={{ padding: 28, textAlign: 'center', color: 'var(--fg-4)' }}>No runs were recorded for this session.</div>
            ) : runs.map((run) => (
              <div key={run.id} className="script-row" style={{ gridTemplateColumns: 'minmax(140px, .7fr) minmax(220px, 1.5fr) 90px 90px' }}>
                <span>
                  <span className="name">{run.name}</span>
                  <span className="mono" style={{ display: 'block', color: 'var(--fg-4)', fontSize: 10.5, marginTop: 2 }}>pid {run.pid ?? '—'}</span>
                </span>
                <span className="cmd" title={run.command}>{run.command}</span>
                <span className={'tag ' + (run.status === 'running' || run.status === 'completed' ? 'ok' : '')}>{run.status}</span>
                <span className="mono" style={{ color: 'var(--fg-3)', fontSize: 11 }}>
                  {formatDuration(Math.max(0, Math.floor(((run.ended_at_ms ?? Date.now()) - run.started_at_ms) / 1000)))}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div></div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ padding: 14, borderRight: '1px solid var(--line-soft)' }}>
      <div className="eyebrow">{label}</div>
      <div style={{ marginTop: 6, color: 'var(--fg-1)', fontSize: 12, overflowWrap: 'anywhere' }}>{value}</div>
    </div>
  );
}
