// Gemeinsamer Sidecar Aufruf Helfer. Inhaltlich wortgleich aus NovaPhonic
// übernommen (dortige main.rs, Funktion `run_sidecar`), nur hierher in ein
// eigenes Modul ausgelagert, damit ihn alle vier Pipeline Schritte
// gemeinsam nutzen können, statt ihn mehrfach zu kopieren. Einzige
// inhaltliche Erweiterung: eine optionale `clip_id`, damit
// Fortschrittsmeldungen bei einem Batch aus mehreren Clips ihrem jeweiligen
// Clip zugeordnet werden können.

use tauri::AppHandle;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::pipeline::emit_progress;

pub async fn run_sidecar(
    app: &AppHandle,
    clip_id: Option<&str>,
    sidecar_name: &str,
    args: Vec<String>,
    step: &str,
) -> Result<(), String> {
    let cmd = app
        .shell()
        .sidecar(sidecar_name)
        .map_err(|e| e.to_string())?
        .args(args);
    let (mut rx, _child) = cmd.spawn().map_err(|e| e.to_string())?;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) | CommandEvent::Stderr(line) => {
                emit_progress(
                    app,
                    clip_id,
                    step,
                    "running",
                    None,
                    Some(String::from_utf8_lossy(&line).trim().to_string()),
                );
            }
            CommandEvent::Error(err) => return Err(err),
            CommandEvent::Terminated(payload) => {
                if payload.code != Some(0) {
                    return Err(format!(
                        "{sidecar_name} wurde mit Code {:?} beendet",
                        payload.code
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}
