// Schritt 4: Zusammenfügen und Rendering in Remotion.
//
// Bekommt das fertige Manifest aus manifest.rs übergeben (alle
// vorbereiteten Videos in der finalen Reihenfolge, je mit eigener
// Schnittliste) und rendert daraus ein einziges finales Gesamtvideo.
//
// Architekturentscheidung Remotion Anbindung, getroffen am 2026-08-29
// gemeinsam mit JJ: Weg a) aus den ursprünglich skizzierten zwei Wegen,
// eine portable Node.js Laufzeit als weiteres Sidecar Programm, zusammen
// mit dem vollständigen, mitgelieferten Remotion Projekt unter
// `tauri-app/remotion/` (Komposition, Renderskript, node_modules). Der
// Grund für diese Wahl gegenüber Weg b (zu einer .exe kompilierter
// Renderer, z.B. per `pkg`): Remotion selbst braucht zur Laufzeit ohnehin
// einen headless Chromium Browser (nicht nur Node.js), dieser wird über
// `node_modules/.remotion/chrome-headless-shell/` mitgeliefert, ein
// kompilierter Einzeldatei Renderer würde daran nichts ändern, aber die
// Wartung erschweren. Details inklusive der bewusst in Kauf genommenen
// Paketgröße (Node plus Chromium, insgesamt eher 300 bis 500 MB) und der
// Remotion Lizenzfrage (unkritisch, KI-WERKE ist ein Einzelunternehmen
// ohne Angestellte) stehen in docs/ARCHITEKTUR.md.
//
// Die eigentliche Sidecar Ausführung läuft strukturell genau wie bei
// auto-editor, deep-filter und ffmpeg über `run_sidecar`, nur zeigt der
// Sidecar Name hier auf die portable Node Laufzeit selbst
// (`binaries/node-<target-triple>`, beim offiziellen Windows Node Build
// bereits von Haus aus eine einzelne, eigenständig lauffähige .exe), das
// eigentliche Renderskript `render.mjs` wird als erstes Argument
// übergeben. Das Skript selbst (mitgeliefert als Tauri Ressource unter
// `resources/remotion/`, siehe `tauri.conf.json`) übernimmt auf der
// Node/Remotion Seite alles Weitere: Manifest und Schnittlisten einlesen,
// die Komposition "novastudiocast-timeline" bündeln (siehe
// `remotion/src/Root.tsx` und `remotion/src/Timeline.tsx") und rendern.
// Vollständig lokal end zu Ende in dieser Sitzung getestet (Testclips per
// ffmpeg erzeugt, echtes Manifest und echte Schnittlisten geschrieben,
// `render.mjs` direkt mit Node ausgeführt), das Ergebnis Video hatte exakt
// die aus den Testdaten erwartete Dauer und Auflösung.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::manifest::RenderManifest;
use crate::sidecar::run_sidecar;

pub async fn run(
    app: &AppHandle,
    _manifest: &RenderManifest,
    manifest_path: &Path,
    batch_work_dir: &Path,
) -> Result<PathBuf, String> {
    let render_script = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("remotion")
        .join("render.mjs");

    if !render_script.exists() {
        return Err(format!(
            "Remotion Renderskript wurde nicht gefunden, erwartet unter {}. Ist der Remotion \
             Projektordner als Ressource mitgebündelt (siehe tauri.conf.json, \
             bundle.resources)?",
            render_script.display()
        ));
    }

    let final_video_path = batch_work_dir.join("novastudiocast_final.mp4");

    run_sidecar(
        app,
        None,
        "node",
        vec![
            render_script.to_string_lossy().to_string(),
            manifest_path.to_string_lossy().to_string(),
            final_video_path.to_string_lossy().to_string(),
        ],
        "render",
    )
    .await?;

    if !final_video_path.exists() {
        return Err(format!(
            "Remotion hat keinen Fehler gemeldet, die erwartete Ausgabedatei {} existiert aber \
             nicht.",
            final_video_path.display()
        ));
    }

    Ok(final_video_path)
}
