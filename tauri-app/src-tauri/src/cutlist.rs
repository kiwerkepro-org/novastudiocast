// Wandelt Auto Editors rohes "v1" Exportformat (siehe cut_analysis.rs für
// den genauen Aufruf) in das eigene, sekundenbasierte NovaStudioCast Format
// um. Der Umweg über eine eigene Zwischenschicht statt Auto Editors
// Rohdaten direkt an Remotion weiterzureichen hat zwei Gründe: erstens
// rechnet Auto Editor in Frame bzw. Timebase Einheiten, während Remotion in
// der eigenen Komposition üblicherweise mit Sekunden oder der eigenen
// Projekt fps arbeitet. Zweitens soll eine spätere Änderung an Auto Editor
// (andere Version, anderes Exportformat) nicht automatisch das für Remotion
// sichtbare Format verändern, die Übersetzung passiert an genau dieser
// einen Stelle.
//
// Vollständige Feldbeschreibung inklusive Beispielen: siehe
// docs/JSON_SCHEMA.md im Projektordner.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Rohformat, wie Auto Editor es mit `--export v1` tatsächlich schreibt,
/// offiziell dokumentiert unter https://auto-editor.com/docs/v1
#[derive(Deserialize)]
struct AutoEditorV1 {
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    source: String,
    /// Rationale Zahl als Text, z.B. "30000/1001" oder "30/1".
    timebase: String,
    /// Jeder Eintrag: [start, end, speed]. start ist einschließlich, end
    /// ausschließlich, beide in Timebase Einheiten (also Frames), nicht in
    /// Sekunden. speed 1.0 bedeutet "unverändert behalten", jeder andere
    /// Wert (in der Praxis meist 99999.0) markiert einen von Auto Editor
    /// erkannten, wegzuschneidenden Abschnitt (Stille bzw. Füllwort).
    chunks: Vec<(u64, u64, f64)>,
}

/// Ein einzelner zu behaltender Abschnitt, in Sekunden, vom Beginn der
/// analysierten Datei aus gezählt.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeepSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// Die normalisierte Schnittliste eines einzelnen Clips. Wird als eigene
/// `<name>.cuts.json` Datei neben der veredelten Videodatei des Clips
/// abgelegt und zusätzlich als Verweis in das Gesamt Manifest für Remotion
/// aufgenommen (siehe manifest.rs).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipCutList {
    pub schema_version: String,
    pub clip_id: String,
    /// Position auf der finalen Zeitleiste, vom Nutzer per Drag and Drop
    /// festgelegt. Doppelt zum Manifest gehalten, damit die Datei auch für
    /// sich allein eindeutig bleibt.
    pub order: u32,
    /// Welche Datei tatsächlich analysiert wurde (Original oder bereits
    /// veredelte Fassung), siehe cut_analysis.rs für die Begründung des
    /// Standardverhaltens.
    pub analyzed_source: PathBuf,
    pub fps: f64,
    pub duration_seconds: f64,
    pub keep_segments: Vec<KeepSegment>,
}

/// Wandelt einen Timebase String im Format "Zähler/Nenner" (z.B.
/// "30000/1001") in eine reine fps Zahl um.
fn parse_timebase(timebase: &str) -> Result<f64, String> {
    let (num, den) = timebase
        .split_once('/')
        .ok_or_else(|| format!("Unerwartetes timebase Format: {timebase}"))?;
    let num: f64 = num
        .parse()
        .map_err(|_| format!("Ungültiger Zähler in timebase: {timebase}"))?;
    let den: f64 = den
        .parse()
        .map_err(|_| format!("Ungültiger Nenner in timebase: {timebase}"))?;
    if den == 0.0 {
        return Err(format!("Nenner 0 in timebase: {timebase}"));
    }
    Ok(num / den)
}

pub fn normalize(
    clip_id: &str,
    order: u32,
    analyzed_source: &Path,
    raw_json: &str,
) -> Result<ClipCutList, String> {
    let raw: AutoEditorV1 = serde_json::from_str(raw_json)
        .map_err(|e| format!("Auto Editor v1 JSON konnte nicht gelesen werden: {e}"))?;
    let fps = parse_timebase(&raw.timebase)?;

    // Nur Abschnitte mit speed 1.0 behalten. Direkt aneinandergrenzende
    // Behalten Abschnitte (z.B. weil dazwischen ein einzelnes Füllwort mit
    // eigenem Chunk lag, dessen Nachbarn beide speed 1.0 haben) werden
    // zusammengefasst, damit Remotion später nicht unnötig viele einzelne
    // Teilclips zusammensetzen muss.
    let mut keep_segments: Vec<KeepSegment> = Vec::new();
    for (start, end, speed) in &raw.chunks {
        if (*speed - 1.0).abs() > f64::EPSILON {
            continue;
        }
        let start_seconds = *start as f64 / fps;
        let end_seconds = *end as f64 / fps;
        if let Some(last) = keep_segments.last_mut() {
            if (last.end_seconds - start_seconds).abs() < 1e-6 {
                last.end_seconds = end_seconds;
                continue;
            }
        }
        keep_segments.push(KeepSegment {
            start_seconds,
            end_seconds,
        });
    }

    // Gesamtlänge der analysierten Datei ergibt sich aus dem Ende des
    // letzten Chunks, Auto Editors chunks dürfen laut Format keine Lücken
    // haben und decken die komplette Datei ab, ein zusätzlicher ffprobe
    // Aufruf ist dafür nicht nötig.
    let duration_seconds = raw
        .chunks
        .last()
        .map(|(_, end, _)| *end as f64 / fps)
        .unwrap_or(0.0);

    Ok(ClipCutList {
        schema_version: "1.0".to_string(),
        clip_id: clip_id.to_string(),
        order,
        analyzed_source: analyzed_source.to_path_buf(),
        fps,
        duration_seconds,
        keep_segments,
    })
}

pub fn write_cut_list(cut_list: &ClipCutList, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cut_list).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

/// Anteil der Clipdauer, ab dem ein einzelnes verbliebenes `keepSegment`
/// als "praktisch keine Stille gefunden" gilt, statt als normaler Clip,
/// der zufällig ganz ohne Schnitt auskommt. 98 Prozent statt exakt 100,
/// falls Auto Editor durch Rundung oder eine Marge am Rand nicht ganz bis
/// ans Ende reicht.
const NO_SILENCE_COVERAGE_THRESHOLD: f64 = 0.98;

/// Erkennt, ob Auto Editor bei diesem Clip praktisch keine Stille
/// gefunden hat: nach dem Zusammenfassen benachbarter Abschnitte bleibt
/// nur ein einziges `keepSegment` übrig, das nahezu die komplette
/// Clipdauer abdeckt. Kein technischer Fehler, aber ein Hinweis, dass
/// entweder die Aufnahme ungewöhnlich laut bzw. rauschig ist oder die
/// Einstellungen (Margin, Entrauschen) nicht zur Aufnahme passen. Von
/// JJ am 2026-08-19 als Warnung gewünscht, wichtig gerade im Batch,
/// damit ein Ausreißer unter vielen Clips nicht übersehen wird.
pub fn likely_no_silence_found(cut_list: &ClipCutList) -> bool {
    if cut_list.duration_seconds <= 0.0 {
        return false;
    }
    match cut_list.keep_segments.as_slice() {
        [only] => {
            let covered = only.end_seconds - only.start_seconds;
            covered >= cut_list.duration_seconds * NO_SILENCE_COVERAGE_THRESHOLD
        }
        _ => false,
    }
}
