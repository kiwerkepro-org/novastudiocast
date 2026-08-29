// Schritt 3: Schnittanalyse mit Auto Editor.
//
// Analysiert ausschließlich, es wird an dieser Stelle noch kein Video
// geschnitten oder gerendert, das passiert für den gesamten Batch erst
// gemeinsam in Schritt 4 durch Remotion. Auto Editor kann als offizielles
// Exportformat seiner eigenen Stilleerkennung eine Rohdatei im "v1" Format
// schreiben (https://auto-editor.com/docs/v1), die genau das enthält, was
// NovaStudioCast hier braucht: eine lückenlose Liste aus Behalten- und
// Wegschneiden-Abschnitten, ohne dass dabei tatsächlich gerendert wird.
// Aufruf entsprechend mit `--export v1 -o <Zieldatei>.json` statt wie in
// NovaPhonic mit `-o <Zieldatei>.<ext>` für eine fertig geschnittene
// Videodatei.
//
// Wichtiger Hinweis zur Quelle der Analyse, mit JJ am 2026-08-19
// entschieden: Im ursprünglichen Projektbriefing war vorgesehen, dass
// dieser Schritt auf der Originaldatei läuft. NovaPhonic hat aber am
// 2026-08-19 gelernt (siehe dortiger Kommentar in main.rs, Abschnitt "1)
// Entrauschen"), dass Auto Editor mit einem festen Lautstärke Schwellwert
// arbeitet und bei durchgehendem Hintergrundgeräusch im Originalton oft
// gar nichts als Stille erkennt, weil das Grundrauschen permanent über
// der Schwelle liegt. Deshalb analysiert NovaStudioCast standardmäßig die
// Ausgabe von Schritt 1 (`AnalyzeSource::PostDenoise`, der Standardwert),
// also die Tonspur direkt nach DeepFilterNet, aber noch vor Equalizer,
// Kompressor und Lautheitsnormierung aus Schritt 2. Deshalb läuft diese
// Analyse im Ablauf (siehe pipeline/mod.rs) auch bewusst VOR
// Audioveredelung, nicht danach. War Entrauschen deaktiviert, zeigt
// `ctx.current` an dieser Stelle unverändert auf die Originaldatei, ganz
// ohne Sonderfall. Das ist unkritisch für die Bildgenauigkeit der
// erkannten Schnittpunkte, weil weder das Entrauschen noch die
// Lautheitsnormierung die Bildspur verändern (beide Schritte nutzen
// `-c:v copy`, der Video Frame Zeitindex bleibt exakt gleich), die
// Zeitstempel passen also gleichermaßen auf Original, entrauschte und
// komplett veredelte Datei. Wer dennoch das Verhalten aus dem
// ursprünglichen Briefing möchte (immer die Originaldatei analysieren,
// auch bei aktivem Entrauschen), stellt `analyzeSource: "original"` in
// den PipelineOptions ein.

use std::path::PathBuf;
use tauri::AppHandle;

use super::emit_progress;
use super::ClipContext;
use crate::cutlist::{likely_no_silence_found, normalize, write_cut_list};
use crate::sidecar::run_sidecar;

/// Ergebnis der Schnittanalyse für genau einen Clip.
pub struct CutAnalysisResult {
    pub cut_list_path: PathBuf,
    pub fps: f64,
    pub duration_seconds: f64,
    /// Siehe `cutlist::likely_no_silence_found`: true, wenn Auto Editor
    /// bei diesem Clip praktisch keine Stille gefunden hat.
    pub no_silence_warning: bool,
}

/// Führt die Schnittanalyse für einen Clip aus und gibt Pfad, fps,
/// Gesamtlänge sowie ein Warnflag zurück. fps und Gesamtlänge werden aus
/// Auto Editors eigenem `timebase` Feld abgeleitet, ein zusätzlicher
/// ffprobe Aufruf ist dafür nicht nötig, siehe cutlist.rs.
pub async fn run(
    app: &AppHandle,
    ctx: &ClipContext,
    analyze_target: &PathBuf,
    margin_seconds: f64,
) -> Result<CutAnalysisResult, String> {
    let clip_id = Some(ctx.clip_id.as_str());
    emit_progress(app, clip_id, "cut-analysis", "running", Some("startet…".into()), None);

    let raw_json_path = ctx.work_dir.join(format!("{}_autoeditor_v1.json", ctx.stem));
    let margin = format!("{margin_seconds}sec");

    // Hinweis wie in NovaPhonics BAUANLEITUNG.md: bitte beim ersten
    // Testlauf prüfen, ob `--export v1` in der installierten auto-editor
    // Version noch so heißt (ältere Versionen kannten dafür z.B.
    // `--export_as_json`).
    run_sidecar(
        app,
        clip_id,
        "auto-editor",
        vec![
            analyze_target.to_string_lossy().to_string(),
            "--margin".into(),
            margin,
            "--export".into(),
            "v1".into(),
            "-o".into(),
            raw_json_path.to_string_lossy().to_string(),
            "--no-open".into(),
        ],
        "cut-analysis",
    )
    .await
    .map_err(|e| {
        emit_progress(app, clip_id, "cut-analysis", "error", Some(e.clone()), None);
        e
    })?;

    let raw_json = std::fs::read_to_string(&raw_json_path)
        .map_err(|e| format!("Auto Editor v1 JSON konnte nicht gelesen werden: {e}"))?;
    let cut_list = normalize(&ctx.clip_id, ctx.order, analyze_target, &raw_json).map_err(|e| {
        emit_progress(app, clip_id, "cut-analysis", "error", Some(e.clone()), None);
        e
    })?;

    let cut_list_path = ctx.work_dir.join(format!("{}.cuts.json", ctx.stem));
    write_cut_list(&cut_list, &cut_list_path)?;

    let no_silence_warning = likely_no_silence_found(&cut_list);
    if no_silence_warning {
        // Bewusst Status "warning" statt "error": die Verarbeitung läuft
        // normal weiter, das soll in der Oberfläche aber auffallen, auch
        // wenn viele andere Clips im selben Batch unauffällig durchlaufen.
        emit_progress(
            app,
            clip_id,
            "cut-analysis",
            "warning",
            Some(
                "Auto Editor hat praktisch keine Stille gefunden, der Clip bleibt nahezu \
                 unverändert. Bitte Aufnahme und Einstellungen (Margin, Entrauschen) prüfen."
                    .into(),
            ),
            None,
        );
    }

    let fps = cut_list.fps;
    let duration_seconds = cut_list.duration_seconds;
    emit_progress(app, clip_id, "cut-analysis", "done", Some("fertig".into()), None);
    Ok(CutAnalysisResult {
        cut_list_path,
        fps,
        duration_seconds,
        no_silence_warning,
    })
}
