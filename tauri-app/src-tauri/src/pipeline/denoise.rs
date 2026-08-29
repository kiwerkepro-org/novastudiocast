// Schritt 1: Rauschunterdrückung mit DeepFilterNet.
//
// Fachlich eins zu eins aus NovaPhonic übernommen (dortige main.rs,
// Abschnitt "1) Entrauschen"), nur an die ClipContext Struktur von
// NovaStudioCast angepasst. Die Werkzeug Aufrufe (ffmpeg Extraktion,
// deep-filter mit --atten-lim-db 30, ffmpeg Remux) bleiben inhaltlich
// unverändert. Für die ausführliche Begründung der einzelnen Parameter
// (u.a. Nutzerfeedback vom 2026-08-19 zum dumpfen Klang im Vergleich zu
// Auphonic, sowie die Erkenntnis, dass deep-filter bei -o einen
// Ausgabeordner statt einer Zieldatei erwartet) siehe die entsprechenden
// Kommentare in NovaPhonics main.rs, hier bewusst nicht dupliziert.

use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

use super::emit_progress;
use super::ClipContext;
use crate::sidecar::run_sidecar;

pub async fn run(app: &AppHandle, ctx: &ClipContext) -> Result<PathBuf, String> {
    let clip_id = Some(ctx.clip_id.as_str());
    emit_progress(app, clip_id, "denoise", "running", Some("startet…".into()), None);

    let raw_audio = ctx.work_dir.join(format!("{}_audio.wav", ctx.stem));
    let denoised_audio = ctx.work_dir.join(format!("{}_entrauscht.wav", ctx.stem));

    run_sidecar(
        app,
        clip_id,
        "ffmpeg",
        vec![
            "-y".into(),
            "-i".into(),
            ctx.current.to_string_lossy().to_string(),
            "-vn".into(),
            "-acodec".into(),
            "pcm_s16le".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "1".into(),
            raw_audio.to_string_lossy().to_string(),
        ],
        "denoise",
    )
    .await
    .map_err(|e| {
        emit_progress(app, clip_id, "denoise", "error", Some(e.clone()), None);
        e
    })?;

    // deep-filter erwartet bei -o einen Ausgabeordner, keine Zieldatei, und
    // schreibt dort eine Datei mit demselben Namen wie die Eingabedatei
    // hinein (per echtem Testlauf in NovaPhonic am 2026-08-19 bestätigt).
    let denoise_out_dir = ctx.work_dir.join(format!("{}_denoise_tmp", ctx.stem));
    fs::create_dir_all(&denoise_out_dir).map_err(|e| e.to_string())?;
    run_sidecar(
        app,
        clip_id,
        "deep-filter",
        vec![
            raw_audio.to_string_lossy().to_string(),
            "--atten-lim-db".into(),
            "30".into(),
            "-o".into(),
            denoise_out_dir.to_string_lossy().to_string(),
        ],
        "denoise",
    )
    .await
    .map_err(|e| {
        emit_progress(app, clip_id, "denoise", "error", Some(e.clone()), None);
        e
    })?;

    let denoise_result = denoise_out_dir.join(
        raw_audio
            .file_name()
            .ok_or_else(|| "Ungültiger Audio Dateiname.".to_string())?,
    );
    fs::rename(&denoise_result, &denoised_audio)
        .or_else(|_| fs::copy(&denoise_result, &denoised_audio).map(|_| ()))
        .map_err(|e| {
            let msg = format!(
                "Entrauschte Datei nicht am erwarteten Ort {} gefunden: {e}",
                denoise_result.display()
            );
            emit_progress(app, clip_id, "denoise", "error", Some(msg.clone()), None);
            msg
        })?;
    let _ = fs::remove_dir_all(&denoise_out_dir);

    let remuxed = ctx
        .work_dir
        .join(format!("{}_entrauscht_video.{}", ctx.stem, ctx.ext));
    run_sidecar(
        app,
        clip_id,
        "ffmpeg",
        vec![
            "-y".into(),
            "-i".into(),
            ctx.current.to_string_lossy().to_string(),
            "-i".into(),
            denoised_audio.to_string_lossy().to_string(),
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            "1:a:0".into(),
            "-c:v".into(),
            "copy".into(),
            "-shortest".into(),
            remuxed.to_string_lossy().to_string(),
        ],
        "denoise",
    )
    .await
    .map_err(|e| {
        emit_progress(app, clip_id, "denoise", "error", Some(e.clone()), None);
        e
    })?;

    let _ = fs::remove_file(&raw_audio);
    let _ = fs::remove_file(&denoised_audio);
    emit_progress(app, clip_id, "denoise", "done", Some("fertig".into()), None);
    Ok(remuxed)
}
