// Saemtliche HTTP Kommunikation mit einer bereits lokal laufenden Ollama
// Instanz (Standardport 11434, https://ollama.com). Ollama selbst wird
// bewusst NICHT gebuendelt und laeuft nicht als Sidecar (siehe
// ../sidecar.rs), sondern muss der Nutzer separat installieren und laufen
// lassen, siehe Projektgedaechtnis "chat_ki_modelle" und
// "monetarisierung" fuer die Begruendung. Dieses Modul ruft deshalb ganz
// normal ueber HTTP auf localhost zu (per reqwest), kein
// tauri-plugin-http noetig, weil ausschliesslich von Rust Code aus
// aufgerufen, nie direkt vom Frontend.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use super::tools::{tool_definitions, ChatContext, ChatReply, ChatToolCall, ChatTurn};
use super::OllamaStatus;

const BASE_URL: &str = "http://127.0.0.1:11434";

fn client_with_timeout(secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(secs))
        .build()
        .expect("reqwest Client konnte nicht erstellt werden")
}

#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

/// Kurzer Erreichbarkeitscheck mit knappem Zeitlimit (2 Sekunden), damit
/// die Oberflaeche nicht laenger haengt, falls Ollama gar nicht laeuft.
pub async fn status() -> OllamaStatus {
    let client = client_with_timeout(2);
    match client.get(format!("{BASE_URL}/api/version")).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<VersionResponse>().await {
            Ok(v) => OllamaStatus {
                running: true,
                version: Some(v.version),
            },
            Err(_) => OllamaStatus {
                running: true,
                version: None,
            },
        },
        _ => OllamaStatus {
            running: false,
            version: None,
        },
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagsModel>,
}
#[derive(Deserialize)]
struct TagsModel {
    name: String,
    #[serde(default)]
    size: u64,
}

pub async fn list_installed() -> Result<Vec<InstalledModel>, String> {
    let client = client_with_timeout(5);
    let resp = client
        .get(format!("{BASE_URL}/api/tags"))
        .send()
        .await
        .map_err(|e| format!("Ollama nicht erreichbar: {e}"))?;
    let parsed: TagsResponse = resp
        .json()
        .await
        .map_err(|e| format!("Antwort von Ollama konnte nicht gelesen werden: {e}"))?;
    Ok(parsed
        .models
        .into_iter()
        .map(|m| InstalledModel {
            name: m.name,
            size_bytes: m.size,
        })
        .collect())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    pub model: String,
    pub status: String,
    pub total: Option<u64>,
    pub completed: Option<u64>,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct PullLine {
    #[serde(default)]
    status: String,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

/// Laedt ein Modell ueber Ollamas streamenden /api/pull Endpunkt herunter,
/// meldet den Fortschritt laufend ueber das Ereignis
/// "ollama-pull-progress" an die Oberflaeche (siehe app.js), unabhaengig
/// vom "pipeline-progress" Ereignis der eigentlichen Videopipeline, weil
/// zeitlich voellig losgeloest von einem Batch Lauf.
pub async fn pull_model(app: &AppHandle, model: &str) -> Result<(), String> {
    // Bewusst kein Zeitlimit auf dem Client: ein Modell Download kann je
    // nach Rechner und Internetanbindung mehrere Minuten dauern, ein
    // festes Timeout wuerde hier faelschlich abbrechen.
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{BASE_URL}/api/pull"))
        .json(&serde_json::json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Ollama nicht erreichbar: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama antwortete mit Status {}", resp.status()));
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Verbindung beim Download unterbrochen: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        // Ollama schickt eine JSON Zeile pro Fortschrittsschritt (NDJSON),
        // Zeilen koennen ueber mehrere Chunks verteilt ankommen, deshalb
        // erst an jedem Zeilenumbruch trennen statt pro Chunk zu parsen.
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer.drain(..=pos);
            if line.is_empty() {
                continue;
            }
            let parsed: PullLine = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if let Some(err) = &parsed.error {
                let _ = app.emit(
                    "ollama-pull-progress",
                    PullProgress {
                        model: model.to_string(),
                        status: parsed.status.clone(),
                        total: parsed.total,
                        completed: parsed.completed,
                        done: true,
                        error: Some(err.clone()),
                    },
                );
                return Err(err.clone());
            }
            let done = parsed.status == "success";
            let _ = app.emit(
                "ollama-pull-progress",
                PullProgress {
                    model: model.to_string(),
                    status: parsed.status.clone(),
                    total: parsed.total,
                    completed: parsed.completed,
                    done,
                    error: None,
                },
            );
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<serde_json::Value>,
    tools: serde_json::Value,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponseToolCallFunction {
    name: String,
    #[serde(default)]
    arguments: serde_json::Value,
}
#[derive(Deserialize)]
struct ChatResponseToolCall {
    function: ChatResponseToolCallFunction,
}
#[derive(Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<ChatResponseToolCall>,
}
#[derive(Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
}

/// Schickt die komplette bisherige Unterhaltung plus eine frisch gebaute
/// Systemnachricht mit dem aktuellen Stand der Oberflaeche (siehe
/// ChatContext::system_prompt) an Ollama, zusammen mit den beiden festen
/// Werkzeugen aus tool_definitions(). Bewusst ohne Streaming (stream:
/// false): auf schwacher Hardware macht ein Tippeffekt kaum einen
/// Unterschied, eine einzige vollstaendige Antwort ist deutlich einfacher
/// zuverlaessig zu verarbeiten, gerade wegen der Werkzeugaufrufe.
pub async fn send_chat(
    model: &str,
    history: Vec<ChatTurn>,
    context: &ChatContext,
) -> Result<ChatReply, String> {
    // Bewusst kein Zeitlimit: gerade auf schwacher CPU kann die Antwort,
    // besonders beim groesseren Qwen3 8B, durchaus eine Weile dauern.
    let client = reqwest::Client::new();

    let mut messages: Vec<serde_json::Value> = Vec::with_capacity(history.len() + 1);
    messages.push(serde_json::json!({ "role": "system", "content": context.system_prompt() }));
    for turn in &history {
        messages.push(serde_json::json!({ "role": turn.role, "content": turn.content }));
    }

    let request = ChatRequest {
        model,
        messages,
        tools: tool_definitions(),
        stream: false,
    };

    let resp = client
        .post(format!("{BASE_URL}/api/chat"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Ollama nicht erreichbar: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama antwortete mit Status {}", resp.status()));
    }

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("Antwort von Ollama konnte nicht gelesen werden: {e}"))?;

    Ok(ChatReply {
        content: parsed.message.content,
        tool_calls: parsed
            .message
            .tool_calls
            .into_iter()
            .map(|tc| ChatToolCall {
                name: tc.function.name,
                arguments: tc.function.arguments,
            })
            .collect(),
    })
}
