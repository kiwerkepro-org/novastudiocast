// Gesamt Manifest, das am Ende der Schritte 1 bis 3 für den kompletten
// Batch einmalig geschrieben und danach unverändert an Remotion in Schritt
// 4 übergeben wird. Enthält alle vorbereiteten Videos in der vom Nutzer per
// Drag and Drop festgelegten Reihenfolge, jeweils zusammen mit dem Pfad zu
// ihrer eigenen Schnittliste aus cutlist.rs.
//
// Vollständige Feldbeschreibung inklusive Beispiel: siehe
// docs/JSON_SCHEMA.md im Projektordner.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub order: u32,
    pub clip_id: String,
    pub processed_video_path: PathBuf,
    pub cut_list_path: PathBuf,
    pub fps: f64,
    pub duration_seconds: f64,
}

/// Eine Warnung zu genau einem Clip im Batch, siehe
/// `cutlist::likely_no_silence_found`. Sammlung aller Warnungen im
/// Manifest, damit die Oberfläche am Ende eine Gesamtübersicht zeigen
/// kann, statt dass eine einzelne Meldung unter vielen Clips im
/// Fortschrittsprotokoll untergeht. Von JJ am 2026-08-19 gewünscht.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchWarning {
    pub clip_id: String,
    pub order: u32,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderManifest {
    pub schema_version: String,
    pub project_id: String,
    /// ISO 8601 Zeitstempel, ausschließlich zur Nachvollziehbarkeit falls
    /// später mehrere Manifeste im selben Arbeitsordner verglichen werden
    /// müssen, für die eigentliche Verarbeitung ohne Bedeutung.
    pub generated_at: String,
    /// Weiche Übergangsdauer in Sekunden, die Remotion an jeder Clipgrenze
    /// anwenden soll, aus `marginSeconds` der PipelineOptions abgeleitet
    /// (siehe pipeline/mod.rs). Ersetzt das `--transition` Flag, das
    /// NovaPhonic direkt an auto-editor übergeben hat, weil auto-editor in
    /// NovaStudioCast in Schritt 3 nicht mehr selbst rendert.
    pub transition_seconds: f64,
    pub timeline: Vec<TimelineEntry>,
    /// Sammelliste aller Clips im Batch, bei denen Auto Editor praktisch
    /// keine Stille gefunden hat. Leer im Normalfall.
    pub warnings: Vec<BatchWarning>,
}

pub fn write_manifest(manifest: &RenderManifest, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
