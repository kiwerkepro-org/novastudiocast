// Definiert die Werkzeuge (Tools), die dem Sprachmodell bei jeder
// /api/chat Anfrage mitgegeben werden, sowie die Antwortstruktur, die von
// hier an das Frontend zurückgeht. Absichtlich schlank gehalten, nur zwei
// Werkzeuge: Einstellungen ändern und den Batch starten. Die Clipliste
// selbst bleibt weiterhin ausschließlich über Drag and Drop im rechten
// Bereich gepflegt (siehe Sitzung 3), das Sprachmodell kann sie weder
// lesen noch verändern, es bekommt nur die Namen zur Orientierung in der
// Systemnachricht. Bewusst nur zwei Werkzeuge statt vieler kleiner, weil
// laut Recherche (siehe Projektgedächtnis "chat_ki_modelle") gerade bei
// kleinen lokalen Modellen die Zuverlässigkeit von Werkzeugaufrufen mit
// der Zahl der angebotenen Werkzeuge sinkt.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    /// "user" oder "assistant", wird unveraendert als Ollama Rolle
    /// weitergereicht.
    pub role: String,
    pub content: String,
}

/// Aktueller Stand der Oberflaeche, wird bei jeder Anfrage frisch als
/// Systemnachricht an das Modell uebergeben, statt ein eigenes Lese
/// Werkzeug anzubieten (siehe Kopfkommentar). Wird komplett vom Frontend
/// aus dem tatsaechlichen DOM Zustand befuellt, siehe app.js,
/// buildChatContext.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatContext {
    pub clip_names: Vec<String>,
    pub denoise: bool,
    pub loudnorm: bool,
    pub loudnorm_target: i32,
    pub margin_seconds: f64,
    pub ai_disclosure_active: bool,
}

impl ChatContext {
    pub fn system_prompt(&self) -> String {
        let clips = if self.clip_names.is_empty() {
            "Aktuell liegen keine Videos in der Liste.".to_string()
        } else {
            format!(
                "Aktuell {} Video(s) in der Liste, in dieser Reihenfolge: {}.",
                self.clip_names.len(),
                self.clip_names.join(", ")
            )
        };
        format!(
            "Du steuerst NovaStudioCast, ein lokales Werkzeug zum automatischen \
             Zusammenschneiden von Videos (Fuellwoerter und Stille werden entfernt, \
             Lautstaerke angeglichen). Antworte auf Deutsch, kurz und klar. Wenn der \
             Nutzer eine Einstellung aendern moechte, rufe set_pipeline_options auf, \
             ausschliesslich mit den Feldern, die sich wirklich aendern sollen. Wenn \
             der Nutzer die Verarbeitung ausdruecklich jetzt starten moechte, rufe \
             start_batch_pipeline auf. Frage lieber kurz nach, statt bei Unklarheit \
             zu raten. Videos selbst kannst du nicht hinzufuegen oder entfernen, das \
             laeuft ausschliesslich per Drag and Drop im rechten Bereich der \
             Oberflaeche, weise bei Bedarf freundlich darauf hin.\n\n\
             {clips} Entrauschen: {denoise}. Lautstaerke angleichen: {loudnorm} (Ziel \
             {target} LUFS). Zeitpuffer um jeden Schnitt: {margin} Sekunden. KI \
             Kennzeichnung nach Artikel 50 EU AI Act: {disclosure}.",
            clips = clips,
            denoise = if self.denoise { "an" } else { "aus" },
            loudnorm = if self.loudnorm { "an" } else { "aus" },
            target = self.loudnorm_target,
            margin = self.margin_seconds,
            disclosure = if self.ai_disclosure_active { "an" } else { "aus" },
        )
    }
}

#[derive(Serialize)]
pub struct ChatToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatReply {
    pub content: String,
    pub tool_calls: Vec<ChatToolCall>,
}

/// JSON Definition der beiden Werkzeuge nach Ollamas /api/chat Format
/// (OpenAI kompatibles "tools" Feld), siehe offizielle Ollama API
/// Dokumentation, docs/api.md im Ollama Repository.
pub fn tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "set_pipeline_options",
                "description": "Aendert eine oder mehrere Einstellungen fuer die Videoverarbeitung. Nur die Felder angeben, die sich wirklich aendern sollen, alle anderen weglassen.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "denoise": {
                            "type": "boolean",
                            "description": "Hintergrundgeraeusche entfernen (DeepFilterNet)."
                        },
                        "loudnorm": {
                            "type": "boolean",
                            "description": "Lautstaerke und Klangbalance angleichen (EBU R128)."
                        },
                        "loudnormTarget": {
                            "type": "integer",
                            "enum": [-16, -23, -14],
                            "description": "Ziel Lautheit in LUFS: -16 Podcast/YouTube, -23 Rundfunk, -14 Streaming laut."
                        },
                        "marginSeconds": {
                            "type": "number",
                            "minimum": 0,
                            "maximum": 1.0,
                            "description": "Zeitpuffer in Sekunden vor und nach jedem Schnitt, zwischen 0 und 1.0."
                        },
                        "aiDisclosureActive": {
                            "type": "boolean",
                            "description": "KI Kennzeichnung nach Artikel 50 EU AI Act ein oder ausschalten."
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "start_batch_pipeline",
                "description": "Startet die Verarbeitung aller aktuell in der Liste befindlichen Videos mit den derzeit aktiven Einstellungen. Nur aufrufen, wenn der Nutzer das ausdruecklich moechte, nicht vorsorglich.",
                "parameters": { "type": "object", "properties": {} }
            }
        }
    ])
}
