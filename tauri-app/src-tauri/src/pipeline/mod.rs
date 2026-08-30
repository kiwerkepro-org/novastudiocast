// Orchestriert alle vier Schritte der NovaStudioCast Pipeline für einen
// kompletten Batch aus einem oder mehreren Videos.
//
// Zentrale Regel im gesamten Modul: strikt sequenziell, ein Arbeitsschritt
// nach dem anderen, nie parallel, damit die Anwendung auch auf einfacher
// Consumer Hardware nicht überlastet. Das gilt sowohl innerhalb eines
// einzelnen Clips (Schritt 1 vor 2 vor 3) als auch zwischen mehreren Clips
// im selben Batch (Clip 1 komplett durch Schritt 1 bis 3, erst danach
// beginnt Clip 2). Schritt 4 läuft am Ende genau einmal für den gesamten
// Batch, nicht pro Clip.

pub mod ai_disclosure;
pub mod audio_refine;
pub mod cut_analysis;
pub mod denoise;
pub mod render;
pub mod types;

use crate::manifest::{write_manifest, BatchWarning, RenderManifest, TimelineEntry};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use types::{AnalyzeSource, BatchResult, ClipJob, ClipManifestEntry, PipelineOptions, ProgressPayload};

pub fn emit_progress(
    app: &AppHandle,
    clip_id: Option<&str>,
    step: &str,
    status: &str,
    detail: Option<String>,
    log: Option<String>,
) {
    use tauri::Emitter;
    let _ = app.emit(
        "pipeline-progress",
        ProgressPayload {
            clip_id: clip_id.map(|s| s.to_string()),
            step: step.to_string(),
            status: status.to_string(),
            detail,
            log,
        },
    );
}

/// Arbeitskontext für einen einzelnen Clip, wird durch die Schritte 1 bis 3
/// weitergereicht und dabei schrittweise befüllt. Entspricht inhaltlich den
/// lokalen Variablen, die NovaPhonics `run_pipeline` bisher einfach als
/// Funktionsparameter durchgereicht hat, hier als eigene Struktur, weil
/// jeder Schritt jetzt eine eigene Datei ist.
pub struct ClipContext {
    pub clip_id: String,
    pub order: u32,
    pub original_input: PathBuf,
    pub work_dir: PathBuf,
    pub stem: String,
    pub ext: String,
    /// Zeigt nach jedem abgeschlossenen Schritt auf die aktuell gültige
    /// Zwischendatei, genau wie `current` in NovaPhonics `run_pipeline`.
    pub current: PathBuf,
}

/// Führt den kompletten Batch aus: für jeden Clip einzeln die Schritte 1
/// bis 3 (siehe `run_clip_steps`), danach für den gesamten Batch einmalig
/// Schritt 4.
///
/// Wichtig: die Reihenfolge der Clips im übergebenen Vec bestimmt NICHT die
/// Position auf der finalen Zeitleiste. Das steuert ausschließlich das Feld
/// `order` in `ClipJob`, das der Nutzer per Drag and Drop in der
/// Oberfläche festlegt. Die Verarbeitungsreihenfolge, also wer zuerst durch
/// Schritt 1 bis 3 läuft, ist davon unabhängig und aktuell einfach die
/// Reihenfolge im Vec, das darf sich später ändern (z.B. kleinste Datei
/// zuerst), ohne dass sich dadurch die spätere Zeitleiste verschiebt.
pub async fn run_batch(
    app: AppHandle,
    jobs: Vec<ClipJob>,
    options: PipelineOptions,
    batch_work_dir: PathBuf,
) -> Result<BatchResult, String> {
    fs::create_dir_all(&batch_work_dir).map_err(|e| e.to_string())?;

    let mut entries: Vec<ClipManifestEntry> = Vec::with_capacity(jobs.len());
    for job in &jobs {
        let entry = run_clip_steps(&app, job, &options, &batch_work_dir).await?;
        entries.push(entry);
    }

    // Reihenfolge für die Zeitleiste erst hier anwenden, unabhängig davon,
    // in welcher Reihenfolge die Clips tatsächlich verarbeitet wurden.
    entries.sort_by_key(|e| e.order);

    let manifest = RenderManifest {
        schema_version: "1.0".to_string(),
        project_id: batch_work_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("novastudiocast-projekt")
            .to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        transition_seconds: options.margin_seconds.min(0.5),
        timeline: entries
            .iter()
            .map(|e| TimelineEntry {
                order: e.order,
                clip_id: e.clip_id.clone(),
                processed_video_path: e.processed_video_path.clone(),
                cut_list_path: e.cut_list_path.clone(),
                fps: e.fps,
                duration_seconds: e.duration_seconds,
            })
            .collect(),
        warnings: entries
            .iter()
            .filter(|e| e.no_silence_warning)
            .map(|e| BatchWarning {
                clip_id: e.clip_id.clone(),
                order: e.order,
                message: "Auto Editor hat praktisch keine Stille gefunden, der Clip bleibt \
                          nahezu unverändert."
                    .to_string(),
            })
            .collect(),
    };

    let manifest_path = batch_work_dir.join("novastudiocast.manifest.json");
    write_manifest(&manifest, &manifest_path)?;

    emit_progress(
        &app,
        None,
        "render",
        "running",
        Some("Remotion startet…".into()),
        None,
    );
    let mut final_video_path = render::run(&app, &manifest, &manifest_path, &batch_work_dir)
        .await
        .map_err(|e| {
            emit_progress(&app, None, "render", "error", Some(e.clone()), None);
            e
        })?;
    emit_progress(&app, None, "render", "done", Some("fertig".into()), None);

    // Schritt 5, optional: KI Kennzeichnung nach Artikel 50 EU AI Act,
    // siehe pipeline/ai_disclosure.rs und pipeline/types.rs,
    // PipelineOptions::ai_disclosure. Laeuft bewusst erst nach dem
    // fertigen Gesamtvideo aus Schritt 4, unabhaengig ein und
    // abschaltbar, aendert nichts an Schritt 1 bis 4. Ersetzt bei
    // aktivem Baustein den zurueckgegebenen `final_video_path` durch die
    // gekennzeichnete Fassung, das unveraenderte Remotion Ergebnis bleibt
    // als Zwischendatei im Arbeitsordner erhalten.
    if let Some(ai_disclosure_options) = &options.ai_disclosure {
        final_video_path =
            ai_disclosure::run(&app, ai_disclosure_options, &final_video_path, &batch_work_dir)
                .await?;
    }

    Ok(BatchResult {
        final_video_path,
        manifest_path,
    })
}

/// Schritte 1 bis 3 für genau einen Clip, streng nacheinander.
///
/// Fachlich aus NovaPhonics `run_pipeline` übernommen, aber umsortiert.
/// Tatsächliche Ausführungsreihenfolge ist Entrauschen, dann
/// Schnittanalyse, dann erst Audioveredelung, also Schritt 1, 3, 2 statt
/// 1, 2, 3 aus dem ursprünglichen Briefing. Grund: Auto Editor erkennt
/// Stille anhand eines festen Lautstärke Schwellwerts und findet bei
/// durchgehendem Hintergrundgeräusch im Originalton oft gar nichts, siehe
/// ausführliche Begründung in cut_analysis.rs. Die Schnittanalyse läuft
/// deshalb möglichst früh, direkt nach dem Entrauschen und noch vor
/// Equalizer, Kompressor und Lautheitsnormierung aus Schritt 2. Das ändert
/// nichts an der Bildgenauigkeit der erkannten Schnittpunkte, weil weder
/// Entrauschen noch Audioveredelung die Bildspur verändern (beide nutzen
/// `-c:v copy`).
async fn run_clip_steps(
    app: &AppHandle,
    job: &ClipJob,
    options: &PipelineOptions,
    batch_work_dir: &PathBuf,
) -> Result<ClipManifestEntry, String> {
    let input = PathBuf::from(&job.input_path);
    if !input.exists() {
        return Err(format!("Eingabedatei nicht gefunden: {}", job.input_path));
    }
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .to_string();
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4")
        .to_string();
    let work_dir = batch_work_dir.join(format!("clip-{}", job.id));
    fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;

    let mut ctx = ClipContext {
        clip_id: job.id.clone(),
        order: job.order,
        original_input: input.clone(),
        work_dir,
        stem,
        ext,
        current: input,
    };

    // Schritt 1: Entrauschen (DeepFilterNet), optional.
    if options.denoise {
        ctx.current = denoise::run(app, &ctx).await?;
    } else {
        emit_progress(
            app,
            Some(&ctx.clip_id),
            "denoise",
            "done",
            Some("übersprungen".into()),
            None,
        );
    }

    // Schritt 3: Schnittanalyse (Auto Editor), immer aktiv, liefert nur
    // eine JSON Datei, kein Video wird hier gerendert. Läuft bewusst schon
    // hier, zwischen Schritt 1 und Schritt 2, siehe Begründung oben und in
    // cut_analysis.rs. Mit dem Standardwert `AnalyzeSource::PostDenoise`
    // zeigt `ctx.current` an dieser Stelle entweder auf die gerade fertig
    // entrauschte Datei (Entrauschen war aktiv) oder unverändert auf die
    // Originaldatei (Entrauschen war deaktiviert), ganz ohne Sonderfall.
    let analyze_target = match options.analyze_source {
        AnalyzeSource::PostDenoise => ctx.current.clone(),
        AnalyzeSource::Original => ctx.original_input.clone(),
    };
    let cut_analysis_result =
        cut_analysis::run(app, &ctx, &analyze_target, options.margin_seconds).await?;

    // Schritt 2: Audioveredelung (FFmpeg EQ und EBU R128 Lautheit),
    // optional. Läuft jetzt nach der Schnittanalyse statt davor, siehe
    // Begründung oben.
    if options.loudnorm {
        ctx.current = audio_refine::run(app, &ctx, options.loudnorm_target).await?;
    } else {
        emit_progress(
            app,
            Some(&ctx.clip_id),
            "loudnorm",
            "done",
            Some("übersprungen".into()),
            None,
        );
    }

    Ok(ClipManifestEntry {
        clip_id: ctx.clip_id,
        order: ctx.order,
        processed_video_path: ctx.current,
        cut_list_path: cut_analysis_result.cut_list_path,
        fps: cut_analysis_result.fps,
        duration_seconds: cut_analysis_result.duration_seconds,
        no_silence_warning: cut_analysis_result.no_silence_warning,
    })
}
