// Schritt 2: Audioveredelung, Equalizer und EBU R128 Lautheitsnormierung
// per FFmpeg.
//
// Fachlich eins zu eins aus NovaPhonic übernommen (dortige main.rs,
// Abschnitt "3) Klangveredelung & Lautstärke"), dort ausführlich begründet
// (externe Analyse Empfehlung zu Hochpassfilter, gezielter 350 Hz
// Absenkung gegen topfigen Klang, 3500 Hz Presence Boost, moderater
// Kompressor, Einzelpass statt Zweipass loudnorm). Diese Begründungen hier
// bewusst nicht dupliziert, siehe NovaPhonics main.rs.
//
// In NovaStudioCast läuft dieser Schritt bewusst NACH der Schnittanalyse
// (Stand 2026-08-19, ursprünglich andersherum geplant), nicht wie in
// NovaPhonic vor dem eigentlichen Schnitt. Grund: Auto Editor soll in
// Schritt 3 die Tonspur direkt nach dem Entrauschen analysieren, noch vor
// Equalizer, Kompressor und Lautheitsnormierung, weil genau diese
// Bearbeitung (vor allem der Kompressor) sonst echte Stille wieder
// anheben und die Stille Erkennung erschweren würde. Siehe
// cut_analysis.rs für die ausführliche Begründung.

use std::path::PathBuf;
use tauri::AppHandle;

use super::emit_progress;
use super::ClipContext;
use crate::sidecar::run_sidecar;

pub async fn run(app: &AppHandle, ctx: &ClipContext, loudnorm_target: i32) -> Result<PathBuf, String> {
    let clip_id = Some(ctx.clip_id.as_str());
    emit_progress(app, clip_id, "loudnorm", "running", Some("startet…".into()), None);

    let refined = ctx.work_dir.join(format!("{}_nova.{}", ctx.stem, ctx.ext));
    let filter = format!(
        "highpass=f=90,equalizer=f=350:t=q:w=1.5:g=-3,equalizer=f=3500:t=q:w=1.2:g=3.5,acompressor=threshold=0.1:ratio=2.5:attack=15:release=200:makeup=1.5,loudnorm=I={}:TP=-1.5:LRA=11",
        loudnorm_target
    );
    run_sidecar(
        app,
        clip_id,
        "ffmpeg",
        vec![
            "-y".into(),
            "-i".into(),
            ctx.current.to_string_lossy().to_string(),
            "-af".into(),
            filter,
            "-c:v".into(),
            "copy".into(),
            refined.to_string_lossy().to_string(),
        ],
        "loudnorm",
    )
    .await
    .map_err(|e| {
        emit_progress(app, clip_id, "loudnorm", "error", Some(e.clone()), None);
        e
    })?;

    emit_progress(app, clip_id, "loudnorm", "done", Some("fertig".into()), None);
    Ok(refined)
}
