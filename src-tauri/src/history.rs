use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const MAX_BYTES: i64 = 250 * 1024 * 1024;

#[derive(Clone)]
pub struct HistoryStore {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySession {
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub title: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub status: String,
    pub service_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRun {
    pub id: String,
    pub session_id: Option<String>,
    pub service_id: String,
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub name: String,
    pub cwd: String,
    pub command: String,
    pub pid: Option<u32>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub status: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryLog {
    pub id: i64,
    pub run_id: String,
    pub service_id: String,
    pub timestamp_ms: i64,
    pub sequence: i64,
    pub stream: String,
    pub message: String,
}

impl HistoryStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                workspace_name TEXT NOT NULL,
                title TEXT NOT NULL,
                started_at_ms INTEGER NOT NULL,
                ended_at_ms INTEGER,
                status TEXT NOT NULL,
                expected_runs INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY,
                session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
                service_id TEXT NOT NULL,
                project_id TEXT,
                workspace_id TEXT,
                name TEXT NOT NULL,
                cwd TEXT NOT NULL,
                command TEXT NOT NULL,
                pid INTEGER,
                started_at_ms INTEGER NOT NULL,
                ended_at_ms INTEGER,
                status TEXT NOT NULL,
                exit_code INTEGER
            );
            CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                service_id TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                sequence INTEGER NOT NULL,
                stream TEXT NOT NULL,
                message TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_runs_session ON runs(session_id, started_at_ms);
            CREATE INDEX IF NOT EXISTS idx_logs_run ON logs(run_id, sequence);
            CREATE INDEX IF NOT EXISTS idx_logs_search ON logs(timestamp_ms, service_id);
            PRAGMA user_version=1;
            ",
        )
        .map_err(|e| e.to_string())?;
        let _ = conn.execute(
            "ALTER TABLE sessions ADD COLUMN expected_runs INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.prune()?;
        Ok(store)
    }

    pub fn create_session(
        &self,
        workspace_id: &str,
        workspace_name: &str,
        title: Option<&str>,
    ) -> Result<HistorySession, String> {
        let started_at_ms = now_ms();
        let id = format!("session-{started_at_ms}-{workspace_id}");
        let title = title
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(workspace_name)
            .to_string();
        self.conn
            .lock()
            .map_err(|e| e.to_string())?
            .execute(
                "INSERT INTO sessions (id, workspace_id, workspace_name, title, started_at_ms, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'active')",
                params![id, workspace_id, workspace_name, title, started_at_ms],
            )
            .map_err(|e| e.to_string())?;
        Ok(HistorySession {
            id,
            workspace_id: workspace_id.to_string(),
            workspace_name: workspace_name.to_string(),
            title,
            started_at_ms,
            ended_at_ms: None,
            status: "active".to_string(),
            service_count: 0,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_run(
        &self,
        run_id: &str,
        session_id: Option<&str>,
        service_id: &str,
        project_id: Option<&str>,
        workspace_id: Option<&str>,
        name: &str,
        cwd: &str,
        command: &str,
        pid: u32,
        started_at_ms: i64,
    ) -> Result<(), String> {
        self.conn
            .lock()
            .map_err(|e| e.to_string())?
            .execute(
                "INSERT INTO runs
                 (id, session_id, service_id, project_id, workspace_id, name, cwd, command, pid, started_at_ms, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'running')",
                params![
                    run_id,
                    session_id,
                    service_id,
                    project_id,
                    workspace_id,
                    name,
                    cwd,
                    command,
                    pid,
                    started_at_ms
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn append_log(
        &self,
        run_id: &str,
        service_id: &str,
        timestamp_ms: i64,
        sequence: i64,
        stream: &str,
        message: &str,
    ) -> Result<(), String> {
        self.conn
            .lock()
            .map_err(|e| e.to_string())?
            .execute(
                "INSERT INTO logs (run_id, service_id, timestamp_ms, sequence, stream, message)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![run_id, service_id, timestamp_ms, sequence, stream, message],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn finish_run(&self, run_id: &str, status: &str, exit_code: Option<i32>) -> Result<(), String> {
        let ended_at_ms = now_ms();
        let session_id: Option<String> = {
            let conn = self.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE runs SET ended_at_ms=?2, status=?3, exit_code=?4 WHERE id=?1",
                params![run_id, ended_at_ms, status, exit_code],
            )
            .map_err(|e| e.to_string())?;
            conn.query_row(
                "SELECT session_id FROM runs WHERE id=?1",
                params![run_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .flatten()
        };
        if let Some(session_id) = session_id {
            self.finish_session_if_idle(&session_id)?;
        }
        Ok(())
    }

    pub fn finish_session_if_idle(&self, session_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let expected: i64 = conn
            .query_row(
                "SELECT expected_runs FROM sessions WHERE id=?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE session_id=?1 AND ended_at_ms IS NULL",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE session_id=?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if expected > 0 && active == 0 && total >= expected {
            conn.execute(
                "UPDATE sessions SET ended_at_ms=?2, status='completed' WHERE id=?1 AND ended_at_ms IS NULL",
                params![session_id, now_ms()],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn finalize_session_setup(&self, session_id: &str, expected_runs: usize) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        if expected_runs == 0 {
            conn.execute(
                "UPDATE sessions SET expected_runs=0, ended_at_ms=?2, status='failed' WHERE id=?1",
                params![session_id, now_ms()],
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }
        conn.execute(
            "UPDATE sessions SET expected_runs=?2 WHERE id=?1",
            params![session_id, expected_runs as i64],
        )
        .map_err(|e| e.to_string())?;
        drop(conn);
        self.finish_session_if_idle(session_id)
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<HistorySession>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.workspace_id, s.workspace_name, s.title, s.started_at_ms,
                        s.ended_at_ms, s.status, COUNT(r.id)
                 FROM sessions s LEFT JOIN runs r ON r.session_id=s.id
                 GROUP BY s.id ORDER BY s.started_at_ms DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let sessions = stmt
            .query_map(params![limit as i64], |row| {
                Ok(HistorySession {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    workspace_name: row.get(2)?,
                    title: row.get(3)?,
                    started_at_ms: row.get(4)?,
                    ended_at_ms: row.get(5)?,
                    status: row.get(6)?,
                    service_count: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(sessions)
    }

    pub fn list_runs(&self, session_id: Option<&str>, limit: usize) -> Result<Vec<HistoryRun>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let sql = if session_id.is_some() {
            "SELECT id, session_id, service_id, project_id, workspace_id, name, cwd, command,
                    pid, started_at_ms, ended_at_ms, status, exit_code
             FROM runs WHERE session_id=?1 ORDER BY started_at_ms ASC LIMIT ?2"
        } else {
            "SELECT id, session_id, service_id, project_id, workspace_id, name, cwd, command,
                    pid, started_at_ms, ended_at_ms, status, exit_code
             FROM runs ORDER BY started_at_ms DESC LIMIT ?2"
        };
        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let map = |row: &rusqlite::Row<'_>| {
            Ok(HistoryRun {
                id: row.get(0)?,
                session_id: row.get(1)?,
                service_id: row.get(2)?,
                project_id: row.get(3)?,
                workspace_id: row.get(4)?,
                name: row.get(5)?,
                cwd: row.get(6)?,
                command: row.get(7)?,
                pid: row.get(8)?,
                started_at_ms: row.get(9)?,
                ended_at_ms: row.get(10)?,
                status: row.get(11)?,
                exit_code: row.get(12)?,
            })
        };
        let rows = if let Some(session_id) = session_id {
            stmt.query_map(params![session_id, limit as i64], map)
        } else {
            stmt.query_map(params![rusqlite::types::Null, limit as i64], map)
        }
        .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn list_logs(
        &self,
        run_id: Option<&str>,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<HistoryLog>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let needle = query
            .filter(|value| !value.is_empty())
            .map(|value| format!("%{value}%"));
        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, service_id, timestamp_ms, sequence, stream, message
                 FROM logs
                 WHERE (?1 IS NULL OR run_id=?1) AND (?2 IS NULL OR message LIKE ?2)
                 ORDER BY timestamp_ms DESC, sequence DESC LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(params![run_id, needle, limit as i64], |row| {
                Ok(HistoryLog {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    service_id: row.get(2)?,
                    timestamp_ms: row.get(3)?,
                    sequence: row.get(4)?,
                    stream: row.get(5)?,
                    message: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows.reverse();
        Ok(rows)
    }

    pub fn clear_completed(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM sessions WHERE ended_at_ms IS NOT NULL", [])
            .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM runs WHERE session_id IS NULL AND ended_at_ms IS NOT NULL",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn prune(&self) -> Result<(), String> {
        let cutoff = now_ms() - MAX_AGE_MS;
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM sessions WHERE ended_at_ms IS NOT NULL AND ended_at_ms < ?1",
            params![cutoff],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM runs WHERE session_id IS NULL AND ended_at_ms IS NOT NULL AND ended_at_ms < ?1",
            params![cutoff],
        )
        .map_err(|e| e.to_string())?;

        loop {
            let stored_bytes: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(length(message) + 96), 0) FROM logs",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if stored_bytes <= MAX_BYTES {
                break;
            }
            let deleted = conn
                .execute(
                    "DELETE FROM sessions WHERE id = (
                        SELECT id FROM sessions WHERE ended_at_ms IS NOT NULL
                        ORDER BY ended_at_ms ASC LIMIT 1
                     )",
                    [],
                )
                .map_err(|e| e.to_string())?;
            if deleted == 0 {
                let deleted_run = conn
                    .execute(
                        "DELETE FROM runs WHERE id = (
                            SELECT id FROM runs
                            WHERE session_id IS NULL AND ended_at_ms IS NOT NULL
                            ORDER BY ended_at_ms ASC LIMIT 1
                         )",
                        [],
                    )
                    .map_err(|e| e.to_string())?;
                if deleted_run == 0 {
                    break;
                }
            }
        }
        Ok(())
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_sessions_runs_and_logs() {
        let store = HistoryStore::open(Path::new(":memory:")).unwrap();
        let session = store.create_session("ws", "Workspace", None).unwrap();
        store
            .start_run(
                "run-1",
                Some(&session.id),
                "svc",
                Some("project"),
                Some("ws"),
                "dev",
                "/tmp",
                "npm run dev",
                42,
                now_ms(),
            )
            .unwrap();
        store
            .append_log("run-1", "svc", now_ms(), 1, "stdout", "ready")
            .unwrap();
        store.finalize_session_setup(&session.id, 1).unwrap();
        store.finish_run("run-1", "completed", Some(0)).unwrap();

        assert_eq!(store.list_sessions(10).unwrap()[0].status, "completed");
        assert_eq!(store.list_runs(Some(&session.id), 10).unwrap().len(), 1);
        assert_eq!(store.list_logs(Some("run-1"), None, 10).unwrap()[0].message, "ready");
    }
}
