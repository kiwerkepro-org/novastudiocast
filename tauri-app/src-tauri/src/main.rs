#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

// NovaStudioCast Rust Controller. Orchestriert die komplette vierstufige
// Pipeline (Entrauschen, Audioveredelung, Schnittanalyse, Remotion
// Rendering) für einen Batch aus einem oder mehreren Videos, strikt
// sequenziell, siehe pipeline/mod.rs für die eigentliche Ablauflogik. Die
// Sidecar Mechanik sowie die Schritte 1 und 2 sind fachlich eins zu eins
// aus NovaPhonic übernommen (siehe dortige main.rs), hier nur auf mehrere
// Clips im Batch und eine dritte, reine Analysestufe statt einer echten
// Schnitt Renderstufe erweitert.
//
// Seit dieser Sitzung zusätzlich: chat/, die lokale Chat KI Anbindung
// (Ollama), siehe chat/mod.rs Kopfkommentar für den Zuschnitt.
//
// Hinweis wie schon bei NovaPhonic (siehe dortige BAUANLEITUNG.md): dieser
// Code wurde von Hand geschrieben und noch nicht kompiliert, der erste
// echte Build ist der erste Kompiliertest. Siehe docs/ARCHITEKTUR.md für
// die Liste der Stellen, die dabei besonders zu prüfen sind.

mod chat;
mod cutlist;
mod manifest;
mod pipeline;
mod sidecar;

use chat::{
    list_chat_models, ollama_installed_models, ollama_pull_model, ollama_send_message,
    ollama_status,
};
use pipeline::types::{BatchResult, ClipJob, PipelineOptions};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PickedFile {
    path: String,
    size_bytes: u64,
}

// Gegenüber NovaPhonics `pick_video_file` (Einzahl) auf Mehrfachauswahl
// erweitert, damit der Nutzer mehrere Clips für einen Batch auf einmal
// auswählen kann. Drag and Drop mehrerer Dateien läuft ohnehin über einen
// eigenen, separaten Weg im Frontend, diese Funktion deckt nur die
// klassische Dateiauswahl per Dialog ab.
#[tauri::command]
fn pick_video_files() -> Vec<PickedFile> {
    let Some(files) = rfd::FileDialog::new()
        .add_filter("Video", &["mp4", "mov", "mkv", "avi", "webm"])
        .pick_files()
    else {
        return Vec::new();
    };
    files
        .into_iter()
        .map(|file| {
            let size = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            PickedFile {
                path: file.to_string_lossy().to_string(),
                size_bytes: size,
            }
        })
        .collect()
}

#[tauri::command]
fn save_output_as(source_path: String, suggested_name: String) -> Result<String, String> {
    let dialog = rfd::FileDialog::new().set_file_name(&suggested_name);
    match dialog.save_file() {
        Some(dest) => {
            fs::copy(&source_path, &dest)
                .map_err(|e| format!("Konnte Datei nicht speichern: {e}"))?;
            Ok(dest.to_string_lossy().to_string())
        }
        None => Err("abgebrochen".to_string()),
    }
}

// Wie bei NovaPhonic: Tauri spricht Sidecar Programme zur Laufzeit nur über
// ihren bloßen Namen an, der Ordnerpfad "binaries/" gehört ausschließlich in
// tauri.conf.json (externalBin) und in die Capabilities.
//
// "node" (Schritt 4, Remotion Rendering) seit 2026-08-29 mit dabei, siehe
// pipeline/render.rs für die getroffene Architekturentscheidung: eine
// portable Node.js Laufzeit als Sidecar, das mitgelieferte Remotion
// Projekt selbst liegt als Ressource unter resources/remotion/, nicht als
// eigenes Sidecar.
//
// Ollama gehört bewusst NICHT in diese Liste: es wird nicht gebündelt,
// sondern vom Nutzer separat installiert und läuft als eigener
// Hintergrunddienst, angesprochen per HTTP statt per Sidecar, siehe
// chat/client.rs.
const SIDECARS: [(&str, &str); 4] = [
    ("auto-editor", "--version"),
    ("deep-filter", "--version"),
    ("ffmpeg", "-version"),
    ("node", "--version"),
];

#[tauri::command]
async fn check_tools(app: AppHandle) -> Vec<String> {
    let mut missing = Vec::new();
    for (sidecar_name, version_flag) in SIDECARS {
        let ok = match app.shell().sidecar(sidecar_name) {
            Ok(cmd) => cmd
                .args([version_flag])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false),
            Err(_) => false,
        };
        if !ok {
            missing.push(sidecar_name.to_string());
        }
    }
    missing
}

// Haupteinstiegspunkt für die Oberfläche: ein kompletter Batch Lauf über
// alle vom Nutzer ausgewählten und in der gewünschten Reihenfolge
// sortierten Clips. Reicht direkt an pipeline::run_batch weiter, siehe dort
// für die eigentliche Ablaufsteuerung.
#[tauri::command]
async fn run_batch_pipeline(
    app: AppHandle,
    jobs: Vec<ClipJob>,
    options: PipelineOptions,
) -> Result<BatchResult, String> {
    if jobs.is_empty() {
        return Err("Kein Video übergeben.".to_string());
    }
    // Eigener Arbeitsordner für den gesamten Batch im System Temp
    // Verzeichnis, aus demselben Grund wie in NovaPhonic (siehe dortige
    // main.rs, Kopfkommentar): Zwischenschritte sollen nicht in einem vom
    // Nutzer synchronisierten Ordner (Nextcloud, OneDrive, etc.) landen.
    let batch_id = format!("novastudiocast-{}", std::process::id());
    let batch_work_dir: PathBuf = std::env::temp_dir().join(batch_id);
    pipeline::run_batch(app, jobs, options, batch_work_dir).await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            pick_video_files,
            save_output_as,
            check_tools,
            run_batch_pipeline,
            list_chat_models,
            ollama_status,
            ollama_installed_models,
            ollama_pull_model,
            ollama_send_message
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Tauri Anwendung");
}
