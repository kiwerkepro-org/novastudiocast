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
    /// Schritt 5, ganz am Ende, optional: sichtbare und maschinenlesbare
    /// KI Kennzeichnung nach Artikel 50 EU AI Act (in Kraft seit
    /// 2026-08-02). Eigener, unabhaengig an und abschaltbarer Baustein
    /// nach dem in Sitzung 2 vereinbarten Options Muster (siehe
    /// MEMORY.md), berührt Schritt 1 bis 4 nicht. `None` = Baustein aus,
    /// `Some(...)` = Baustein an mit den jeweiligen Einstellungen. Siehe
    /// pipeline/ai_disclosure.rs fuer die eigentliche Umsetzung.
    ///
    /// Rechtlicher Hinweis, keine Rechtsberatung: ob und in welchem
    /// Umfang Artikel 50 auf NovaStudioCasts eigene Bearbeitungsschritte
    /// zutrifft, ist eine juristische Einschaetzung, siehe MEMORY.md,
    /// Abschnitt "KI Kennzeichnung, Sitzung 4". Dieses Modul macht die
    /// Kennzeichnung moeglich, ersetzt aber keine eigene Pruefung durch
    /// den Nutzer, ob und wie er sie einsetzt.
    #[serde(default)]
    pub ai_disclosure: Option<AiDisclosureOptions>,
}

/// Einstellungen fuer Schritt 5, siehe `PipelineOptions::ai_disclosure`
/// und `pipeline/ai_disclosure.rs`.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDisclosureOptions {
    pub text: AiDisclosureText,
    /// Nur relevant, wenn `text` == `AiDisclosureText::Custom`.
    #[serde(default)]
    pub custom_text: Option<String>,
    pub position: AiDisclosurePosition,
    pub timing: AiDisclosureTiming,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiDisclosureText {
    KiGeneriert,
    KiUeberarbeitet,
    Custom,
}

impl AiDisclosureText {
    /// Loest den tatsaechlich einzublendenden Text auf. Bei `Custom` mit
    /// leerem oder fehlendem `custom_text` (sollte das Frontend eigentlich
    /// verhindern) faellt der Baustein auf "KI ueberarbeitet" zurueck,
    /// statt eine leere Einblendung zu erzeugen.
    pub fn resolve(&self, custom_text: &Option<String>) -> String {
        match self {
            AiDisclosureText::KiGeneriert => "KI generiert".to_string(),
            AiDisclosureText::KiUeberarbeitet => "KI überarbeitet".to_string(),
            AiDisclosureText::Custom => custom_text
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "KI überarbeitet".to_string()),
        }
    }
}

/// Position der Einblendung, immer am unteren Bildrand, siehe
/// `pipeline/ai_disclosure.rs` fuer den festen Mindestabstand von 20
/// Pixeln zum jeweiligen Rand (JJ, Sitzung 4: "von links 20 Pixel
/// mindestens eingerückt... von rechts 20 Pixel").
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiDisclosurePosition {
    UntenLinks,
    UntenMitte,
    UntenRechts,
}

/// Sichtbarkeitsfenster der Einblendung, komplett vom Nutzer bestimmt, JJ
/// wollte hier ausdruecklich nichts fest vorgeben (Sitzung 4,
/// 2026-08-30): "Er kann das von mir das ganze Video durchlassen oder er
/// kann sagen, nee, [...] mache das nach 20 Sekunden fuer 30 Sekunden und
/// dann soll es wieder weg sein."
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDisclosureTiming {
    pub start_reference: AiDisclosureStartReference,
    /// Sekunden Abstand vom jeweiligen Bezugspunkt (Anfang oder Ende des
    /// fertigen Gesamtvideos).
    pub start_offset_seconds: f64,
    /// `None` = laeuft ab dem Startpunkt bis zum Ende des Videos durch.
    #[serde(default)]
    pub duration_seconds: Option<f64>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiDisclosureStartReference {
    Anfang,
    Ende,
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
