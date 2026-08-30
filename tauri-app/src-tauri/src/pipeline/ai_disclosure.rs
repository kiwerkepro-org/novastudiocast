// Schritt 5 (optional, ganz am Ende): sichtbare und maschinenlesbare KI
// Kennzeichnung nach Artikel 50 EU AI Act, auf dem fertigen Gesamtvideo
// aus Schritt 4 (Remotion). Läuft nur, wenn `PipelineOptions.ai_disclosure`
// gesetzt ist (siehe pipeline/types.rs), unabhängig ein und abschaltbar,
// ändert nichts an Schritt 1 bis 4.
//
// Entscheidungen aus Sitzung 4 (2026-08-30), siehe MEMORY.md für den
// vollständigen Wortlaut:
// - Eigenes NovaStudioCast Modul statt Weiterreichen an NovaImage.
// - Drei Textvorlagen ("KI generiert", "KI überarbeitet") plus eigener
//   Text.
// - Position immer am unteren Bildrand (links, Mitte oder rechts), mit
//   mindestens 20 Pixel Abstand zum jeweiligen Rand, JJ ausdrücklich:
//   "von links 20 Pixel mindestens eingerückt ... von unten 20 Pixel
//   mindestens eingerückt ... von rechts 20 Pixel".
// - Sichtbarkeitsfenster (wann die Einblendung beginnt und wie lange sie
//   bleibt) ist komplett frei vom Nutzer bestimmbar, nicht fest
//   vorgegeben, siehe AiDisclosureTiming in pipeline/types.rs.
// - Zusätzlich zur sichtbaren Einblendung wird ein maschinenlesbarer
//   Hinweis als MP4 Metadaten Tag (`comment`) mit eingebettet, Artikel 50
//   Absatz 2 zielt ausdrücklich auf maschinenlesbare Kennzeichnung ab.
//
// Rechtlicher Hinweis, keine Rechtsberatung: siehe ausführlicher Kommentar
// bei `PipelineOptions::ai_disclosure` in pipeline/types.rs sowie
// MEMORY.md.
//
// Technischer Unterschied zu den anderen optionalen Nachbearbeitungs
// Schritten (Untertitel, Kapitelmarken, siehe MEMORY.md, Sitzung 2): jene
// lassen sich verlustfrei per Remux einbetten (`-c copy`), weil sie nur
// zusätzliche Spuren bzw. Metadaten hinzufügen. Eine sichtbar ins Bild
// eingebrannte Einblendung verändert dagegen die Bildpixel selbst, das
// Video muss also neu kodiert werden (`-c:v libx264`), nur die Tonspur
// bleibt per `-c:a copy` unangetastet.
//
// Alle drei FFmpeg Aufrufe/Escaping Regeln in diesem Modul wurden vor dem
// Schreiben dieses Kommentars an einem echten, lokal erzeugten Testvideo
// mit einem simulierten Windows Pfad (Laufwerksbuchstabe plus
// Doppelpunkt) durchgetestet, nicht nur von Hand geschrieben, siehe
// Testprotokoll in der Sitzung. Dabei kam eine wichtige, vorher nicht
// bekannte FFmpeg Eigenheit heraus: siehe `escape_ffmpeg_filter_path`
// unten.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use super::emit_progress;
use super::types::{AiDisclosureOptions, AiDisclosurePosition, AiDisclosureStartReference};
use crate::sidecar::run_sidecar;

/// Ermittelt die Gesamtdauer der fertig gerenderten Datei in Sekunden.
/// Gebraucht für den Bezugspunkt "Ende" beim Sichtbarkeitsfenster
/// (`AiDisclosureStartReference::Ende`). Das Gesamt Manifest kennt zwar
/// die Dauer jedes einzelnen Clips VOR dem Rendering (siehe manifest.rs),
/// nicht aber die tatsächliche Gesamtdauer NACH den Überblendungen aus
/// `transitionSeconds` (die verkürzen die Summe der Einzeldauern etwas).
/// Deshalb hier eine eigene, robuste Abfrage direkt auf der fertigen
/// Datei.
///
/// Bewusst nicht über `run_sidecar`: `ffmpeg -i <datei>` ohne
/// Ausgabedatei beendet sich planmäßig mit Code 1 (kein Fehler, ffmpeg
/// gibt nur seine Eingabeanalyse aus und bricht danach ab, weil keine
/// Ausgabe angegeben wurde), `run_sidecar` würde das fälschlich als
/// Fehlschlag werten. Deshalb hier ein eigener, einfacher Aufruf, der nur
/// den Text nach "Duration:" ausliest, unabhängig vom Exit Code.
async fn probe_duration_seconds(app: &AppHandle, video_path: &Path) -> Result<f64, String> {
    let cmd = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| e.to_string())?
        .args(["-i", &video_path.to_string_lossy()]);
    let output = cmd.output().await.map_err(|e| e.to_string())?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_duration_line(&combined).ok_or_else(|| {
        format!(
            "Konnte die Gesamtdauer von {} nicht aus der ffmpeg Ausgabe lesen.",
            video_path.display()
        )
    })
}

/// Sucht "Duration: HH:MM:SS.ss" in ffmpegs Standardausgabe (Stderr) und
/// rechnet es in Sekunden um.
fn parse_duration_line(text: &str) -> Option<f64> {
    let marker = "Duration: ";
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find(',')?;
    let stamp = &rest[..end];
    let parts: Vec<&str> = stamp.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: f64 = parts[0].trim().parse().ok()?;
    let minutes: f64 = parts[1].trim().parse().ok()?;
    let seconds: f64 = parts[2].trim().parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Wandelt einen Dateipfad in die von FFmpeg Filtergraphen erwartete Form
/// um: Rückwärts zu Schrägstrichen, und der Doppelpunkt nach einem
/// Windows Laufwerksbuchstaben wird zusätzlich mit Backslash escaped,
/// weil Doppelpunkte im Filtergraphen sonst als Optionstrenner gelesen
/// werden (betrifft `fontfile=` und `textfile=` unten, beides Optionen
/// mit absoluten Dateipfaden als Wert).
///
/// Wichtig, per echtem Testlauf bestätigt: das alleine reicht NICHT,
/// zusätzlich muss der escapte Pfad noch in einfache Anführungszeichen
/// gesetzt werden (siehe `run` unten, `fontfile='...'`). Ohne die
/// Anführungszeichen hat FFmpeg beim Testen den escapten Doppelpunkt an
/// einer Stelle im Filtergraphen falsch mitgezählt, sobald `fontfile`
/// direkt von einer weiteren `key=value` Option (hier `textfile=`)
/// gefolgt wurde: FFmpeg brach dann mit der Fehlermeldung "Both text and
/// text file provided" ab, obwohl gar kein `text=` gesetzt war. Mit
/// Anführungszeichen um den escapten Pfad trat der Fehler in keinem der
/// Testläufe mehr auf.
fn escape_ffmpeg_filter_path(path: &Path) -> String {
    let forward = path.to_string_lossy().replace('\\', "/");
    forward.replacen(':', "\\:", 1)
}

pub async fn run(
    app: &AppHandle,
    options: &AiDisclosureOptions,
    rendered_video_path: &Path,
    batch_work_dir: &Path,
) -> Result<PathBuf, String> {
    emit_progress(
        app,
        None,
        "ai-disclosure",
        "running",
        Some("ermittle Videolänge…".into()),
        None,
    );
    let total_seconds = probe_duration_seconds(app, rendered_video_path)
        .await
        .map_err(|e| {
            emit_progress(app, None, "ai-disclosure", "error", Some(e.clone()), None);
            e
        })?;

    let start_seconds = match options.timing.start_reference {
        AiDisclosureStartReference::Anfang => options.timing.start_offset_seconds.max(0.0),
        AiDisclosureStartReference::Ende => {
            (total_seconds - options.timing.start_offset_seconds).max(0.0)
        }
    };
    let end_seconds = options
        .timing
        .duration_seconds
        .map(|duration| (start_seconds + duration).min(total_seconds));

    let enable_expr = match end_seconds {
        Some(end) => format!("between(t,{start_seconds},{end})"),
        None => format!("gte(t,{start_seconds})"),
    };

    let label_text = options.text.resolve(&options.custom_text);

    // Text über eine eigene UTF-8 Datei einbinden (`textfile=`), statt ihn
    // direkt in den Filtergraphen zu schreiben. So scheitern weder
    // Umlaute (ä, ö, ü, ß) noch ein vom Nutzer frei eingegebener eigener
    // Text an den Sonderzeichen, die FFmpeg Filtergraphen sonst selbst
    // escaped haben wollen (Doppelpunkt, Komma, Anführungszeichen).
    let text_file = batch_work_dir.join("ki_kennzeichnung_text.txt");
    std::fs::write(&text_file, &label_text)
        .map_err(|e| format!("Konnte Kennzeichnungstext nicht schreiben: {e}"))?;

    // Schriftdatei als Tauri Ressource, siehe tauri.conf.json
    // (`bundle.resources`) und docs/ARCHITEKTUR.md, Abschnitt "KI
    // Kennzeichnung (Schritt 5)". Es ist dieselbe Inter Schrift, die auch
    // die Oberfläche selbst benutzt (app/assets/fonts, OFL Lizenz), aus
    // der bereits vorhandenen Variable Font Datei (.woff2) einmalig als
    // eigenständige .ttf herausgelöst, damit FFmpeg/Freetype sie laden
    // kann (Windows Ffmpeg Builds haben in aller Regel kein Fontconfig
    // und keine WOFF2 Unterstützung in Freetype eingebaut, brauchen also
    // eine echte .ttf Datei mit explizitem Pfad).
    let font_path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("fonts")
        .join("Inter-Regular.ttf");

    let (x_expr, y_expr) = match options.position {
        AiDisclosurePosition::UntenLinks => ("20".to_string(), "h-text_h-20".to_string()),
        AiDisclosurePosition::UntenMitte => {
            ("(w-text_w)/2".to_string(), "h-text_h-20".to_string())
        }
        AiDisclosurePosition::UntenRechts => {
            ("w-text_w-20".to_string(), "h-text_h-20".to_string())
        }
    };

    let drawtext = format!(
        "drawtext=fontfile='{}':textfile='{}':fontsize=28:fontcolor=white:\
         box=1:boxcolor=black@0.55:boxborderw=10:x={}:y={}:enable='{}'",
        escape_ffmpeg_filter_path(&font_path),
        escape_ffmpeg_filter_path(&text_file),
        x_expr,
        y_expr,
        enable_expr
    );

    let end_label = end_seconds
        .map(|end| format!("bis {end:.1}s"))
        .unwrap_or_else(|| "bis Videoende".to_string());
    let metadata_value = format!(
        "KI Hinweis nach Artikel 50 EU AI Act: {label_text} (Sichtbarkeitsfenster {start_seconds:.1}s {end_label})"
    );

    let labeled_video_path = batch_work_dir.join("novastudiocast_final_ki_kennzeichnung.mp4");

    emit_progress(
        app,
        None,
        "ai-disclosure",
        "running",
        Some("blendet Kennzeichnung ein…".into()),
        None,
    );

    run_sidecar(
        app,
        None,
        "ffmpeg",
        vec![
            "-y".into(),
            "-i".into(),
            rendered_video_path.to_string_lossy().to_string(),
            "-vf".into(),
            drawtext,
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "medium".into(),
            "-crf".into(),
            "18".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "copy".into(),
            "-metadata".into(),
            format!("comment={metadata_value}"),
            labeled_video_path.to_string_lossy().to_string(),
        ],
        "ai-disclosure",
    )
    .await
    .map_err(|e| {
        emit_progress(app, None, "ai-disclosure", "error", Some(e.clone()), None);
        e
    })?;

    let _ = std::fs::remove_file(&text_file);

    if !labeled_video_path.exists() {
        let msg = format!(
            "FFmpeg hat keinen Fehler gemeldet, die erwartete Ausgabedatei {} existiert aber \
             nicht.",
            labeled_video_path.display()
        );
        emit_progress(app, None, "ai-disclosure", "error", Some(msg.clone()), None);
        return Err(msg);
    }

    emit_progress(app, None, "ai-disclosure", "done", Some("fertig".into()), None);
    Ok(labeled_video_path)
}
