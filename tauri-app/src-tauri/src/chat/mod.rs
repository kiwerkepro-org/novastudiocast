// Chat KI Anbindung, lokale Stufe. Reine Steuerung per Textbefehl ueber
// eine bereits vom Nutzer installierte und laufende Ollama Instanz, siehe
// client.rs fuer die HTTP Kommunikation und tools.rs fuer die beiden dem
// Modell angebotenen Werkzeuge sowie die Antwortstruktur.
//
// Wichtige Abgrenzung: dieses Modul fuehrt Werkzeugaufrufe NICHT selbst
// aus. Es reicht die vom Modell gewuenschten Aufrufe (set_pipeline_options
// / start_batch_pipeline) unveraendert an das Frontend weiter, siehe
// ChatReply::tool_calls. Grund: die Clipliste (Pfade, Reihenfolge) lebt
// ausschliesslich im Frontend (app.js, Variable `clips`), eine zweite
// Kopie hier im Rust Code haette dieselbe Information doppelt und aus dem
// Tritt geraten koennen. Das Frontend wendet einen Werkzeugaufruf auf
// dieselbe Weise an, wie es ein manueller Klick auf die jeweiligen
// Bedienelemente auch taete.
//
// Eine spaetere, separat geplante kostenpflichtige Cloud/API Stufe (siehe
// Projektgedaechtnis "monetarisierung") soll sich hier als zweite,
// austauschbare Implementierung neben client.rs einreihen lassen, ohne
// dass sich am Zuschnitt dieses Moduls oder an tools.rs etwas aendern
// muss.

pub mod client;
pub mod tools;

use serde::Serialize;
use tauri::AppHandle;

pub use tools::{ChatContext, ChatReply, ChatTurn};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub running: bool,
    pub version: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelOption {
    /// Name, wie Ollama ihn kennt, wird unveraendert an "ollama pull" bzw.
    /// den /api/pull Endpunkt uebergeben.
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub ram_hint: &'static str,
}

/// Feste Auswahl von vier Modellen, in dieser Reihenfolge von JJ
/// festgelegt, siehe Projektgedaechtnis "chat_ki_modelle" fuer die
/// Begruendung, Quellen und die genauen RAM Werte. Bewusst als feste
/// Liste im Rust Code statt einer vom Nutzer frei eintippbaren Modell
/// Auswahl, damit nur Modelle angeboten werden, die fuer diesen
/// Anwendungsfall (Textbefehl zu Werkzeugaufruf) und fuer eher schwache
/// Hardware tatsaechlich geprueft sind.
pub const CHAT_MODELS: [ChatModelOption; 4] = [
    ChatModelOption {
        id: "gemma4:e2b",
        label: "Gemma 4 E2B",
        description: "Schlankste Wahl, laeuft auf praktisch jedem Rechner mit, guter \
                       Einstieg oder Absicherung bei wenig Arbeitsspeicher.",
        ram_hint: "ca. 1,5 GB RAM",
    },
    ChatModelOption {
        id: "gemma4:e4b",
        label: "Gemma 4 E4B",
        description: "Setzt Befehle genauer um als E2B, braucht dafuer etwas mehr \
                       Arbeitsspeicher.",
        ram_hint: "ca. 3 bis 4 GB RAM",
    },
    ChatModelOption {
        id: "phi4-mini",
        label: "Phi 4 mini",
        description: "Von Microsoft, stark bei klarer Logik und sauberer Struktur, mit \
                       echter Funktionsaufruf Unterstuetzung.",
        ram_hint: "ca. 2,5 GB RAM",
    },
    ChatModelOption {
        id: "qwen3:8b",
        label: "Qwen3 8B",
        description: "Gilt aktuell als das zuverlaessigste der vier bei echten \
                       Funktionsaufrufen, dafuer der groesste Brocken, eher fuer \
                       staerkere Rechner.",
        ram_hint: "ca. 8 bis 12 GB RAM",
    },
];

#[tauri::command]
pub fn list_chat_models() -> Vec<ChatModelOption> {
    CHAT_MODELS.to_vec()
}

#[tauri::command]
pub async fn ollama_status() -> OllamaStatus {
    client::status().await
}

#[tauri::command]
pub async fn ollama_installed_models() -> Result<Vec<client::InstalledModel>, String> {
    client::list_installed().await
}

#[tauri::command]
pub async fn ollama_pull_model(app: AppHandle, model: String) -> Result<(), String> {
    client::pull_model(&app, &model).await
}

#[tauri::command]
pub async fn ollama_send_message(
    model: String,
    history: Vec<ChatTurn>,
    context: ChatContext,
) -> Result<ChatReply, String> {
    client::send_chat(&model, history, &context).await
}
