// Gemeinsame Datentypen für die NovaStudioCast Pipeline. Getrennt von der
// eigentlichen Schrittlogik gehalten, damit main.rs, die einzelnen
// Pipeline Schritte und das Manifest Modul dieselben Strukturen verwenden,
// ohne sich gegenseitig zu importieren.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Vom Frontend übergebene Grundeinstellungen für einen kompletten Batch
/// Lauf. Gilt gleichermaßen für alle Clips im Batch, individuelle
/// Einstellungen pro Clip sind für die erste Ausbaustufe bewusst nicht
/// vorgesehen.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineOptions {
    pub denoise: bool,
    pub loudnorm: bool,
    pub loudnorm_target: i32,
    pub margin_seconds: f64,
    /// Auf welcher Tonspur Auto Editor die Stille Erkennung durchführt.
    /// Siehe ausführlicher Kommentar in cut_analysis.rs. Standard ist
    /// `PostDenoise`, nicht `Original`, aus demselben Grund, aus dem
    /// NovaPhonic den Schnitt schon immer erst nach dem Entrauschen laufen
    /// lässt.
    #[serde(default)]
    pub analyze_source: AnalyzeSource,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AnalyzeSource {
    /// Analysiert die Ausgabe von Schritt 1 (Entrauschen), also nach
    /// DeepFilterNet, aber noch vor Equalizer, Kompressor und
    /// Lautheitsnormierung aus Schritt 2. War Entrauschen deaktiviert,
    /// läuft die Analyse automatisch auf der Originaldatei, ohne dass
    /// dafür ein Sonderfall nötig ist, siehe pipeline/mod.rs.
    #[default]
    PostDenoise,
    /// Analysiert immer die ursprüngliche Eingabedatei, unabhängig davon,
    /// ob Entrauschen aktiv ist. Entspricht dem ursprünglichen
    /// Projektbriefing, siehe cut_analysis.rs für das Risiko bei
    /// durchgehendem Hintergrundgeräusch.
    Original,
}

/// Eine einzelne Eingabedatei im Batch, inklusive ihrer vom Nutzer per
/// Drag and Drop festgelegten Position auf der finalen Zeitleiste.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipJob {
    /// Frei wählbare, im Batch eindeutige Kennung, vom Frontend vergeben
    /// (z.B. beim Ablegen im Chat), wird als Ordnername im Arbeitsverzeichnis
    /// sowie in allen Fortschrittsmeldungen und im Manifest verwendet.
    pub id: String,
    pub input_path: String,
    /// Position auf der finalen Zeitleiste, 0 basiert, vom Nutzer über die
    /// Reihenfolge Liste auf der rechten Seite der Oberfläche festgelegt.
    pub order: u32,
}

/// Fortschritts Ereignis für die Oberfläche. Gegenüber NovaPhonic um
/// `clip_id` erweitert, damit die Oberfläche bei einem Batch aus mehreren
/// Clips erkennen kann, zu welchem Clip eine Meldung gehört, auch wenn die
/// Verarbeitung selbst strikt nacheinander läuft. `None` bei `clip_id`
/// steht für ein batchweites Ereignis, aktuell nur bei Schritt 4 (Render),
/// der für den gesamten Batch gemeinsam läuft.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub clip_id: Option<String>,
    pub step: String,
    pub status: String,
    pub detail: Option<String>,
    pub log: Option<String>,
}

/// Ergebnis eines einzelnen Clips nach den Schritten 1 bis 3. Wird am Ende
/// des Batches zusammen mit den Ergebnissen aller anderen Clips zum
/// Gesamt Manifest für Remotion zusammengefügt, siehe manifest.rs.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipManifestEntry {
    pub clip_id: String,
    pub order: u32,
    pub processed_video_path: PathBuf,
    pub cut_list_path: PathBuf,
    pub fps: f64,
    pub duration_seconds: f64,
    /// Siehe `cutlist::likely_no_silence_found`. Wird am Ende von
    /// `run_batch` zu einer Sammelliste aller Warnungen im Batch
    /// zusammengeführt, siehe `manifest::RenderManifest::warnings`.
    pub no_silence_warning: bool,
}

/// Gesamtergebnis eines Batch Laufs nach Schritt 4, wird als Ergebnis des
/// `run_batch_pipeline` Tauri Commands an die Oberfläche zurückgegeben.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub final_video_path: PathBuf,
    pub manifest_path: PathBuf,
}
