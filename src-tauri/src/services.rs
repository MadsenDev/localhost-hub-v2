use serde::Serialize;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};

use crate::history::{now_ms, HistoryStore};

#[derive(Default)]
pub struct ServiceManager {
    children: Arc<Mutex<HashMap<String, ManagedProcess>>>,
}

#[derive(Clone)]
struct ManagedProcess {
    child: Arc<Mutex<Child>>,
    cwd: String,
    cmd: String,
    pid: u32,
    started_at_ms: i64,
    run_id: String,
}

#[derive(Clone, Serialize)]
pub struct ManagedServiceInfo {
    pub service_id: String,
    pub cwd: String,
    pub cmd: String,
    pub pid: u32,
    pub started_at_ms: i64,
    pub run_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceEventKind {
    Started,
    Stdout,
    Stderr,
    Exited,
    Error,
    Stopped,
}

#[derive(Clone, Serialize)]
pub struct ServiceEvent {
    pub service_id: String,
    pub run_id: String,
    pub kind: ServiceEventKind,
    pub message: String,
    pub pid: Option<u32>,
    pub code: Option<i32>,
    pub timestamp_ms: i64,
    pub sequence: i64,
    pub stream: String,
}

impl ServiceManager {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &self,
        app: AppHandle,
        service_id: String,
        cwd: String,
        cmd: String,
        name: Option<String>,
        project_id: Option<String>,
        workspace_id: Option<String>,
        session_id: Option<String>,
        history: HistoryStore,
    ) -> Result<u32, String> {
        {
            let mut children = self.children.lock().map_err(|e| e.to_string())?;
            if let Some(existing) = children.get(&service_id) {
                let mut existing = existing.child.lock().map_err(|e| e.to_string())?;
                match existing.try_wait() {
                    Ok(None) => return Err("service is already running".to_string()),
                    Ok(Some(_)) | Err(_) => {
                        drop(existing);
                        children.remove(&service_id);
                    }
                }
            }
        }

        let mut command = Command::new("sh");
        command
            .arg("-lc")
            .arg(&cmd)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to start `{cmd}` in {cwd}: {e}"))?;

        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let started_at_ms = now_ms();
        let run_id = format!("run-{started_at_ms}-{pid}");
        let sequence = Arc::new(AtomicI64::new(0));

        if let Err(err) = history.start_run(
            &run_id,
            session_id.as_deref(),
            &service_id,
            project_id.as_deref(),
            workspace_id.as_deref(),
            name.as_deref().unwrap_or(&service_id),
            &cwd,
            &cmd,
            pid,
            started_at_ms,
        ) {
            let _ = terminate_child(&mut child);
            let _ = child.wait();
            return Err(format!("failed to initialize run history: {err}"));
        }

        emit(
            &app,
            ServiceEvent {
                service_id: service_id.clone(),
                run_id: run_id.clone(),
                kind: ServiceEventKind::Started,
                message: format!("started `{cmd}`"),
                pid: Some(pid),
                code: None,
                timestamp_ms: started_at_ms,
                sequence: 0,
                stream: "lifecycle".to_string(),
            },
        );

        if let Some(stdout) = stdout {
            spawn_reader(
                app.clone(),
                history.clone(),
                service_id.clone(),
                run_id.clone(),
                stdout,
                ServiceEventKind::Stdout,
                "stdout",
                sequence.clone(),
            );
        }
        if let Some(stderr) = stderr {
            spawn_reader(
                app.clone(),
                history.clone(),
                service_id.clone(),
                run_id.clone(),
                stderr,
                ServiceEventKind::Stderr,
                "stderr",
                sequence.clone(),
            );
        }

        let child = Arc::new(Mutex::new(child));
        self.children
            .lock()
            .map_err(|e| e.to_string())?
            .insert(service_id.clone(), ManagedProcess {
                child: child.clone(),
                cwd,
                cmd,
                pid,
                started_at_ms,
                run_id: run_id.clone(),
            });

        spawn_exit_watcher(
            app,
            history,
            self.children.clone(),
            service_id,
            run_id,
            child,
            pid,
            sequence,
        );

        Ok(pid)
    }

    pub fn list(&self) -> Result<Vec<ManagedServiceInfo>, String> {
        let children = self.children.lock().map_err(|e| e.to_string())?;
        Ok(children
            .iter()
            .map(|(service_id, managed)| ManagedServiceInfo {
                service_id: service_id.clone(),
                cwd: managed.cwd.clone(),
                cmd: managed.cmd.clone(),
                pid: managed.pid,
                started_at_ms: managed.started_at_ms,
                run_id: managed.run_id.clone(),
            })
            .collect())
    }

    pub fn stop(
        &self,
        app: AppHandle,
        service_id: String,
        history: HistoryStore,
    ) -> Result<(), String> {
        let managed = self
            .children
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&service_id)
            .ok_or_else(|| "service is not managed by Localhost Hub".to_string())?;

        let mut child = managed.child.lock().map_err(|e| e.to_string())?;
        let pid = child.id();
        terminate_child(&mut child)?;
        let _ = child.wait();
        history.finish_run(&managed.run_id, "stopped", None)?;
        emit(
            &app,
            ServiceEvent {
                service_id,
                run_id: managed.run_id,
                kind: ServiceEventKind::Stopped,
                message: "stopped".to_string(),
                pid: Some(pid),
                code: None,
                timestamp_ms: now_ms(),
                sequence: 0,
                stream: "lifecycle".to_string(),
            },
        );
        Ok(())
    }
}

fn terminate_child(child: &mut Child) -> Result<(), String> {
    #[cfg(unix)]
    {
        let pgid = format!("-{}", child.id());
        Command::new("kill")
            .args(["-TERM", &pgid])
            .status()
            .map_err(|e| e.to_string())?;
        for _ in 0..30 {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Command::new("kill")
            .args(["-KILL", &pgid])
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        child.kill().map_err(|e| e.to_string())
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_reader<R>(
    app: AppHandle,
    history: HistoryStore,
    service_id: String,
    run_id: String,
    reader: R,
    kind: ServiceEventKind,
    stream: &'static str,
    sequence: Arc<AtomicI64>,
)
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            match line {
                Ok(message) => {
                    let timestamp_ms = now_ms();
                    let sequence = sequence.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = history.append_log(
                        &run_id,
                        &service_id,
                        timestamp_ms,
                        sequence,
                        stream,
                        &message,
                    );
                    emit(&app, ServiceEvent {
                        service_id: service_id.clone(),
                        run_id: run_id.clone(),
                        kind: kind.clone(),
                        message,
                        pid: None,
                        code: None,
                        timestamp_ms,
                        sequence,
                        stream: stream.to_string(),
                    });
                }
                Err(err) => {
                    let timestamp_ms = now_ms();
                    let sequence = sequence.fetch_add(1, Ordering::Relaxed) + 1;
                    emit(
                        &app,
                        ServiceEvent {
                            service_id: service_id.clone(),
                            run_id: run_id.clone(),
                            kind: ServiceEventKind::Error,
                            message: err.to_string(),
                            pid: None,
                            code: None,
                            timestamp_ms,
                            sequence,
                            stream: "lifecycle".to_string(),
                        },
                    );
                    break;
                }
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn spawn_exit_watcher(
    app: AppHandle,
    history: HistoryStore,
    children: Arc<Mutex<HashMap<String, ManagedProcess>>>,
    service_id: String,
    run_id: String,
    child: Arc<Mutex<Child>>,
    pid: u32,
    sequence: Arc<AtomicI64>,
) {
    std::thread::spawn(move || {
        loop {
            let status = {
                let mut child = match child.lock() {
                    Ok(child) => child,
                    Err(err) => {
                        emit(
                            &app,
                            ServiceEvent {
                                service_id: service_id.clone(),
                                run_id: run_id.clone(),
                                kind: ServiceEventKind::Error,
                                message: err.to_string(),
                                pid: Some(pid),
                                code: None,
                                timestamp_ms: now_ms(),
                                sequence: sequence.fetch_add(1, Ordering::Relaxed) + 1,
                                stream: "lifecycle".to_string(),
                            },
                        );
                        break;
                    }
                };
                match child.try_wait() {
                    Ok(Some(status)) => Some(Ok(status.code())),
                    Ok(None) => None,
                    Err(err) => Some(Err(err.to_string())),
                }
            };

            match status {
                Some(Ok(code)) => {
                    let should_emit = if let Ok(mut children) = children.lock() {
                        children.remove(&service_id).is_some()
                    } else {
                        true
                    };
                    if !should_emit {
                        break;
                    }
                    let status = if code.unwrap_or(1) == 0 { "completed" } else { "crashed" };
                    let _ = history.finish_run(&run_id, status, code);
                    emit(
                        &app,
                        ServiceEvent {
                            service_id,
                            run_id,
                            kind: ServiceEventKind::Exited,
                            message: format!("exited with code {}", code.map_or_else(|| "signal".to_string(), |c| c.to_string())),
                            pid: Some(pid),
                            code,
                            timestamp_ms: now_ms(),
                            sequence: sequence.fetch_add(1, Ordering::Relaxed) + 1,
                            stream: "lifecycle".to_string(),
                        },
                    );
                    break;
                }
                Some(Err(message)) => {
                    let should_emit = if let Ok(mut children) = children.lock() {
                        children.remove(&service_id).is_some()
                    } else {
                        true
                    };
                    if !should_emit {
                        break;
                    }
                    let _ = history.finish_run(&run_id, "failed", None);
                    emit(
                        &app,
                        ServiceEvent {
                            service_id,
                            run_id,
                            kind: ServiceEventKind::Error,
                            message,
                            pid: Some(pid),
                            code: None,
                            timestamp_ms: now_ms(),
                            sequence: sequence.fetch_add(1, Ordering::Relaxed) + 1,
                            stream: "lifecycle".to_string(),
                        },
                    );
                    break;
                }
                None => std::thread::sleep(Duration::from_millis(500)),
            }
        }
    });
}

fn emit(app: &AppHandle, event: ServiceEvent) {
    let _ = app.emit("service://event", event);
}
