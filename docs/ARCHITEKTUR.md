# NovaStudioCast, Architekturübersicht Rust Controller

Stand: 2026-08-19, erster Entwicklungsauftrag. Dieses Dokument beschreibt
die Grundstruktur des Rust Controllers, der die vierstufige Pipeline für
einen Batch aus einem oder mehreren Videos orchestriert.

## Ausgangslage

Basis ist der bestehende, funktionsfähige Code von NovaPhonic
(`C:\KIW-SCHMIEDE\NOVA-PHONIC`), das bereits die Schritte Entrauschen
(DeepFilterNet), Klangveredelung und Lautheitsnormierung (FFmpeg) sowie
Schnitt (Auto Editor) lokal als Sidecar Programme ausführt. Die
Sidecar Mechanik (`run_sidecar`) sowie die konkreten FFmpeg und
DeepFilterNet Aufrufe wurden unverändert übernommen, siehe Kommentare in
den jeweiligen Dateien für die Herkunft.

## Was sich gegenüber NovaPhonic ändert

- NovaPhonic verarbeitet ein Video und schneidet es dabei direkt mit
  Auto Editor fertig. NovaStudioCast verarbeitet einen Batch aus mehreren
  Videos, Auto Editor schneidet in Schritt 3 nichts mehr, sondern liefert
  nur noch eine Schnittliste als JSON.
- Die tatsächliche Zusammenführung inklusive präziser Schnitte übernimmt
  neu Remotion in Schritt 4, für den gesamten Batch auf einmal.
- Tatsächliche Ausführungsreihenfolge pro Clip ist Entrauschen, dann
  Schnittanalyse, dann Audioveredelung, also Schritt 1, 3, 2 statt 1, 2, 3
  aus dem ursprünglichen Briefing (Stand 2026-08-19, mit JJ entschieden).
  Grund: Auto Editor erkennt Stille anhand eines festen Lautstärke
  Schwellwerts und findet bei durchgehendem Hintergrundgeräusch im
  Originalton oft nichts, das behebt das Entrauschen zuverlässig. Die
  Schnittanalyse läuft deshalb möglichst früh, direkt nach dem
  Entrauschen und noch vor Equalizer, Kompressor und
  Lautheitsnormierung aus Schritt 2, weil gerade der Kompressor sonst
  echte Stille wieder anheben und die Erkennung erschweren würde. Siehe
  `pipeline/cut_analysis.rs` für die ausführliche Begründung.

## Modulstruktur (`tauri-app/src-tauri/src/`)

```
main.rs              Tauri Commands, Sidecar Prüfung, Einstiegspunkt
sidecar.rs            gemeinsamer Sidecar Aufruf Helfer (aus NovaPhonic)
cutlist.rs             Auto Editor "v1" Rohformat -> eigenes Cutlist Format
manifest.rs            Gesamt Manifest für Remotion (Schritt 4 Eingabe)
pipeline/
  mod.rs                Orchestrator: run_batch, run_clip_steps
  types.rs               PipelineOptions, ClipJob, ProgressPayload, usw.
  denoise.rs              Schritt 1, DeepFilterNet (aus NovaPhonic)
  audio_refine.rs         Schritt 2, FFmpeg EQ + Loudnorm (aus NovaPhonic)
  cut_analysis.rs         Schritt 3, Auto Editor Analyse, neu
  render.rs               Schritt 4, ruft das Remotion Renderskript per Node Sidecar auf
  ai_disclosure.rs         Schritt 5, optional, KI Kennzeichnung Artikel 50 EU AI Act
```

Dazu, außerhalb von `src-tauri`, das eigenständige Node/React Projekt unter
`tauri-app/remotion/` (siehe Abschnitt "Remotion Anbindung" unten):

```
tauri-app/remotion/
  package.json          Abhängigkeiten: remotion, @remotion/bundler,
                         @remotion/renderer, @remotion/transitions,
                         serve-handler, react, react-dom
  render.mjs             Von node.exe aufgerufenes Renderskript, liest
                          Manifest und Schnittlisten, bündelt und rendert
  src/
    index.ts              registriert RemotionRoot beim Bündeln
    Root.tsx               registriert die Komposition "novastudiocast-timeline"
    Timeline.tsx            die eigentliche Komposition, Überblendungen
                            per @remotion/transitions
    types.ts                gemeinsame TypeScript Typen (PlaybackSegment usw.)
```

Diese Aufteilung in ein Modul pro Pipeline Schritt ist bewusst feiner als
NovaPhonics einzelne `main.rs`, damit jeder Schritt für sich lesbar,
testbar und austauschbar bleibt, gerade weil Schritt 4 (Remotion) noch
offen ist und die anderen drei Schritte davon unberührt bleiben sollen.

## Ablaufsteuerung

Der zentrale Grundsatz aus dem Briefing, strikt sequenziell und
ressourcenschonend, ist wörtlich umgesetzt:

1. `run_batch` iteriert über alle Clips im Batch nacheinander in einer
   einfachen `for` Schleife, kein `tokio::spawn`, keine Parallelität.
2. Für jeden einzelnen Clip laufen die Schritte 1, 2 und 3 ebenfalls streng
   nacheinander (`run_clip_steps`), jeder Schritt wartet auf das Ergebnis
   des vorherigen.
3. Erst wenn wirklich alle Clips die Schritte 1 bis 3 durchlaufen haben,
   startet Schritt 4 genau einmal für den gesamten Batch.

Die Reihenfolge, in der Clips verarbeitet werden, ist bewusst von der
Position auf der finalen Zeitleiste getrennt (`ClipJob.order` versus die
Position im übergebenen Vec). Verarbeitungsreihenfolge und
Abspielreihenfolge müssen nicht identisch sein, aktuell sind sie es zwar
noch (einfache Reihenfolge im Vec), das kann sich aber unabhängig ändern
(z.B. kürzeste Datei zuerst verarbeiten), ohne die Zeitleiste zu
beeinflussen.

## Remotion Anbindung, Architekturentscheidung und Umsetzung (2026-08-29)

`pipeline/render.rs` war bis zu dieser Sitzung ein reiner Platzhalter mit
zwei skizzierten Wegen. Beim Testen in dieser Sitzung stellte sich eine
wichtige, vorher nicht bekannte Tatsache heraus: `@remotion/renderer`
braucht zum Rendern selbst bei reiner Node.js Nutzung zusätzlich einen
headless Chromium Browser (`chrome-headless-shell`), das ist keine
Eigenheit der Studio Oberfläche, sondern gilt auch für die
programmatische `renderMedia`/`selectComposition` API. Dieser Browser wird
separat heruntergeladen (in dieser Sitzung automatisch beim ersten
Testlauf, ca. 92 MB für Linux) und landet im Cache Ordner
`node_modules/.remotion/chrome-headless-shell/`. Das war JJ vor dieser
Sitzung nicht bekannt und wurde ihm mitgeteilt, zusammen mit der
zusätzlichen Tatsache, dass Remotions Lizenz ab mehr als 3 Mitarbeitenden
in der nutzenden Organisation eine kostenpflichtige Firmenlizenz
verlangt. JJ hat daraufhin entschieden, wie ursprünglich geplant mit
Remotion weiterzumachen (KI-WERKE ist ein Einzelunternehmen ohne
Angestellte, die Lizenzfrage ist damit unkritisch), und sich für Weg a)
der beiden ursprünglich skizzierten Wege entschieden:

- **Sidecar:** eine portable Node.js Laufzeit, sidecar Name `node`. Der
  offizielle Windows Build von Node.js liefert `node.exe` bereits als
  eigenständig lauffähige einzelne Datei, passt also genau in das
  bestehende Sidecar Muster wie bei auto-editor, deep-filter und ffmpeg.
  Muss lediglich unter `binaries/node-x86_64-pc-windows-msvc.exe`
  abgelegt werden, siehe `tauri.conf.json`, `externalBin`.
- **Ressource statt Sidecar:** das vollständige Remotion Projekt
  (`tauri-app/remotion/`, siehe Modulstruktur oben) wird nicht als
  Sidecar, sondern als normale Tauri Ressource gebündelt
  (`tauri.conf.json`, `bundle.resources`, Zielordner `resources/remotion/`
  in der installierten Anwendung), weil es aus vielen Dateien besteht
  (Skript, Komposition, node_modules inklusive Chromium Cache), nicht aus
  einer einzelnen ausführbaren Datei.
- `pipeline/render.rs` löst den Ressourcenpfad zur Laufzeit über
  `app.path().resource_dir()` auf, prüft, dass `render.mjs` dort
  tatsächlich existiert, und ruft dann `run_sidecar(app, None, "node",
  vec![render_script, manifest_path, final_video_path], "render")` auf,
  strukturell identisch zu den anderen drei Werkzeugen.
- `render.mjs` selbst übernimmt auf der Node Seite: Gesamt Manifest und
  alle einzelnen Schnittlisten von der Festplatte lesen (siehe
  `docs/JSON_SCHEMA.md`), daraus eine flache, aufgelöste Liste aus
  Wiedergabe Abschnitten bauen, einen eigenen kleinen lokalen HTTP Server
  für den Arbeitsordner starten (Remotions Kompositionen laufen im
  Browser Kontext des headless Chromium und haben deshalb bewusst keinen
  Zugriff auf beliebige absolute Dateisystempfade, offizielle Remotion
  Einschränkung, siehe `https://www.remotion.dev/docs/miscellaneous/absolute-paths`),
  die Komposition "novastudiocast-timeline" bündeln und schließlich
  rendern. Details und Feldbedeutungen siehe Kopfkommentar in
  `render.mjs` selbst.
- Warum kein zu einer .exe kompilierter Renderer (Weg b): der Chromium
  Bedarf besteht so oder so, ein `pkg` kompiliertes Programm würde daran
  nichts ändern, aber die Wartung erschweren (eigener Kompilierschritt für
  jedes Remotion Versions Update).

**Vollständig lokal getestet in dieser Sitzung:** zwei kurze Testclips per
ffmpeg erzeugt, eine echte Schnittliste sowie ein echtes Gesamt Manifest
von Hand nach dem Schema aus `docs/JSON_SCHEMA.md` geschrieben, `render.mjs`
direkt mit Node ausgeführt (ohne den Rust Controller, nur das Skript für
sich). Ergebnis: ein technisch korrektes MP4 mit exakt der aus den
Testdaten erwarteten Gesamtdauer (198 Frames bei 30 fps, entspricht der
Summe der Testabschnitte abzüglich der beiden Überblendungen) und der
erwarteten Auflösung (1920x1080). Damit ist die Kernlogik nachweislich
funktionsfähig, nicht nur entworfen.

**Für einen echten Build auf einem Windows Rechner noch zu erledigen,**
analog zu NovaPhonics `BAUANLEITUNG.md` für die anderen Sidecars:

1. Offizielles Windows `node.exe` besorgen, unter
   `tauri-app/src-tauri/binaries/node-x86_64-pc-windows-msvc.exe` ablegen.
2. Im Ordner `tauri-app/remotion/` einmalig `npm install` ausführen (wird
   bewusst nicht mit ins Projekt bzw. auf das Gerät übertragen, siehe
   `.gitignore`, node_modules wird lokal neu erzeugt, unter anderem weil
   der in dieser Sitzung heruntergeladene Chromium Cache für Linux gilt,
   nicht für Windows).
3. Danach einmal `node -e "require('@remotion/renderer').ensureBrowser()"`
   (dieselbe Funktion, die render.mjs beim allerersten echten Rendern
   ohnehin automatisch aufrufen würde) ausführen, damit der zu Windows
   passende `chrome-headless-shell` in `node_modules/.remotion/`
   heruntergeladen wird, BEVOR das Projekt für die Installation der
   Endnutzer gebündelt wird. Ziel ist, dass Endnutzer selbst keine
   Internetverbindung zum ersten Rendern brauchen, ganz im Sinne von
   "vollständig lokal".

**Das oben ist inzwischen erledigt, siehe nächster Abschnitt:** Schritte 2
und 3 laufen ab sofort automatisch in der GitHub Actions Baupipeline,
niemand muss sie mehr von Hand ausführen.

## Windows Installer per GitHub Actions, erledigt (2026-08-29)

JJ hat nachgefragt, ob bereits eine echte `setup.exe` existiert. Antwort
zu diesem Zeitpunkt: nein, es gab nur nachweislich kompilierenden
Quellcode. Diese Entwicklungsumgebung selbst kann keine echte, geprüft
funktionierende Windows Installationsdatei erzeugen, aus zwei Gründen:
sie läuft unter Linux ohne Möglichkeit, das Ergebnis auf echtem Windows zu
testen, und der Zugriff auf github.com ist aus dieser Umgebung heraus
eingeschränkt (nur das reine Git Protokoll funktioniert probeweise, das
Herunterladen einzelner Programme wie auto-editor oder DeepFilterNet von
GitHub Releases aus nicht zuverlässig).

Die Lösung, die JJ ausgewählt hat: derselbe Weg, den NovaPhonic bereits
erfolgreich nutzt, eine GitHub Actions Baupipeline, die auf einem echten
Windows Server bei GitHub läuft. `.github/workflows/build.yml` wurde
direkt von NovaPhonics eigenem, bereits produktiv laufendem Workflow
(`C:\KIW-SCHMIEDE\NOVA-PHONIC\.github\workflows\build.yml`) übernommen und
um zwei Schritte ergänzt:

1. Node Abhängigkeiten des Remotion Projekts installieren (`npm ci` in
   `tauri-app/remotion`).
2. Den zu Windows passenden Chromium für Remotion vorab herunterladen
   (`node -e "require('@remotion/renderer').ensureBrowser()"`), damit er
   zusammen mit dem restlichen `node_modules` Ordner über
   `tauri.conf.json` (`bundle.resources`) direkt mit in die `setup.exe`
   gepackt wird.

Der Sidecar Download Schritt wurde um ein viertes Programm ergänzt:
`node.exe`, direkt von `https://nodejs.org/dist/latest-iron/win-x64/node.exe`
(offizielle Node.js Distribution, funktioniert als eigenständige Datei
ohne Installation, siehe Begründung im Abschnitt oben).

Eine strukturelle Abweichung von NovaPhonic musste dabei berücksichtigt
werden: NovaStudioCast hat unter `tauri-app/` keine eigene `package.json`
(das Frontend unter `app/` ist reines HTML/CSS/JS ohne Bundler), NovaPhonic
dagegen schon (dort baut ein `npm run tauri build` Skript). Der Workflow
für NovaStudioCast installiert die Tauri CLI deshalb direkt über Cargo
(`cargo install tauri-cli`) und baut mit `cargo tauri build` statt über ein
npm Skript.

**Wichtig, ehrlich benannt:** dieser Workflow wurde sorgfältig aus einem
bereits produktiv laufenden Vorbild abgeleitet und die neuen Schritte
wurden gegen echte, erreichbare URLs geprüft (`nodejs.org` Downloadpfad
bestätigt erreichbar), er wurde aber mangels Windows Umgebung und mangels
GitHub Zugriff aus dieser Entwicklungsumgebung heraus noch NICHT selbst
ausgeführt. Der erste echte Lauf nach dem Einrichten des Repositories
(siehe `GITHUB_SETUP.md`) ist der erste echte Test der gesamten Kette.
## KI Kennzeichnung, Schritt 5 (Sitzung 4, 2026-08-30)

Optionaler fünfter Schritt, läuft nur wenn `PipelineOptions.ai_disclosure`
gesetzt ist (siehe `pipeline/types.rs`), direkt nach dem fertigen
Gesamtvideo aus Schritt 4, unabhängig ein und abschaltbar. Entscheidungen
mit JJ (vollständiger Wortlaut in `MEMORY.md`, Abschnitt "KI
Kennzeichnung, Sitzung 4"):

- Eigenes NovaStudioCast Modul (`pipeline/ai_disclosure.rs`), nicht an
  NovaImage weitergereicht.
- Drei Textvorlagen ("KI generiert", "KI überarbeitet") plus ein eigener,
  frei eingegebener Text.
- Position immer am unteren Bildrand (links, Mitte, rechts), mit
  mindestens 20 Pixel Abstand zum jeweiligen Rand.
- Sichtbarkeitsfenster (wann die Einblendung beginnt, wie lange sie
  bleibt) komplett frei vom Nutzer bestimmbar, kein fester Standardwert,
  siehe `AiDisclosureTiming` in `pipeline/types.rs`.
- Zusätzlich zur sichtbaren Einblendung ein maschinenlesbarer Hinweis als
  MP4 Metadaten Tag (`comment`).

Technisch: die sichtbare Einblendung wird per FFmpeg `drawtext` direkt in
die Bildpixel eingebrannt, das Video muss dafür anders als bei Untertitel
oder Kapitelmarken (geplant als verlustfreier Remux) neu kodiert werden
(`libx264`), nur die Tonspur bleibt per `-c:a copy` unangetastet. Das
unveränderte Remotion Ergebnis aus Schritt 4 bleibt zusätzlich als
Zwischendatei im Arbeitsordner erhalten, `BatchResult.final_video_path`
zeigt bei aktivem Baustein auf die neue, gekennzeichnete Datei
(`novastudiocast_final_ki_kennzeichnung.mp4`).

Die Gesamtdauer des fertigen Videos (gebraucht für den Bezugspunkt "Ende"
beim Sichtbarkeitsfenster) wird per `ffmpeg -i` und Auslesen der
"Duration:" Zeile ermittelt, nicht aus dem Manifest, weil die
`transitionSeconds` Überblendungen aus Schritt 4 die Summe der
Einzeldauern verkürzen und die Manifest Daten das nicht abbilden.

**Schriftdatei für FFmpeg drawtext:** Windows FFmpeg Builds haben in der
Regel weder Fontconfig noch WOFF2 Unterstützung in Freetype eingebaut,
drawtext braucht also eine echte `.ttf` Datei mit explizitem Pfad. Statt
eine neue Schrift zu beschaffen, wurde die bereits im Projekt vorhandene,
lizenzierte Inter Schrift (`app/assets/fonts/Inter-Variable.woff2`, OFL
Lizenz) einmalig mit `fontTools` aus dem WOFF2 Container herausgelöst
(`f.flavor = None; f.save(...)`, dieselben Glyphen, nur andere
Verpackung) und liegt jetzt als `tauri-app/src-tauri/resources/fonts/
Inter-Regular.ttf` (plus Lizenztext) im Projekt, eingetragen in
`tauri.conf.json` unter `bundle.resources` (`"resources/fonts": "fonts"`).
Zur Laufzeit über `app.path().resource_dir().join("fonts").join(
"Inter-Regular.ttf")` aufgelöst, genau wie das Remotion Renderskript.

**FFmpeg Filtergraph Escaping, per echtem Testlauf bestätigt:** Windows
Pfade enthalten nach dem Laufwerksbuchstaben einen Doppelpunkt, der im
FFmpeg Filtergraphen sonst als Optionstrenner gelesen wird. Reines
Escaping des Doppelpunkts (`C\:/...`) reicht dabei NICHT aus, wenn direkt
danach eine weitere `key=value` Option im selben Filter folgt (hier
`fontfile=...:textfile=...`): FFmpeg brach in diesem Fall mit "Both text
and text file provided" ab, obwohl gar kein `text=` gesetzt war. Erst mit
zusätzlichen einfachen Anführungszeichen um den escapten Pfad
(`fontfile='C\:/...'`) trat der Fehler in keinem Testlauf mehr auf, siehe
`escape_ffmpeg_filter_path` in `pipeline/ai_disclosure.rs` für die
Umsetzung und den ausführlichen Kommentar dort. Getestet an einem lokal
erzeugten Testvideo mit einem simulierten Windows Pfad (echter
Verzeichnisname mit Doppelpunkt), inklusive sichtbarer Kontrolle des
gerenderten Frames (Text mit Umlaut "KI überarbeitet" korrekt lesbar,
richtig positioniert, halbtransparente Box im Hintergrund) und
`ffprobe` Kontrolle des Metadaten Tags. Nicht getestet: der echte
Windows FFmpeg Build aus der Baupipeline selbst, nur ein lokaler Linux
FFmpeg mit vergleichbarer Konfiguration (`--enable-libfreetype`
vorhanden). Sollte beim ersten echten Windows Baulauf stichprobenartig
gegengeprüft werden, siehe "Offene Punkte" unten.

`cargo check` sowie `cargo clippy --all-targets` liefen mit diesem neuen
Modul in dieser Sitzung erneut fehlerfrei durch (Linux Testumgebung mit
denselben Tauri Systempaketen wie in Sitzung 3, Sidecar Platzhalterdateien
für `x86_64-unknown-linux-gnu`), keine neuen Fehler oder inhaltlichen
Warnungen gegenüber dem bereits bekannten Stand.

## Offene Punkte für den nächsten Entwicklungsschritt

1. **`--export v1` Flagname prüfen.** Wie schon bei NovaPhonics
   `--no-open`, sollte beim ersten echten Testlauf geprüft werden, ob die
   installierte Auto Editor Version dieses Flag noch so nennt.
2. **`analyzeSource` Standardwert, entschieden (2026-08-19).** Der
   Controller analysiert standardmäßig die Ausgabe von Schritt 1
   (`AnalyzeSource::PostDenoise`), nicht die ursprüngliche Datei, siehe
   ausführliche Begründung in `pipeline/cut_analysis.rs`. Das weicht vom
   ursprünglichen Briefing (Analyse der Originaldatei) ab, ist aber über
   `analyzeSource: "original"` jederzeit umstellbar.
3. **Frontend noch nicht angefasst.** Das zweigeteilte Layout (Chat links,
   dynamischer Kontextbereich mit Reihenfolge Liste rechts) sowie die
   optische Angleichung an NovaPhonic (siehe `app/style.css` dort) stehen
   noch aus.
4. **Cargo.toml, Icons, tauri.conf.json, Capabilities, weitgehend erledigt
   (2026-08-29).** `tauri.conf.json` und `capabilities/default.json` wurden
   strukturell von NovaPhonic übernommen und für NovaStudioCast angepasst
   (Fenstertitel, Bundle Kennung, `externalBin` Einträge für inzwischen
   alle vier Sidecars inklusive `node`, `bundle.resources` für das
   Remotion Projekt). Die Icons sind vorerst noch die unveränderten
   NovaPhonic Dateien, rein als Platzhalter, damit das Projekt überhaupt
   baubar ist, echte NovaStudioCast eigene Icons stehen noch aus.
5. **KI Kennzeichnung (Schritt 5), FFmpeg drawtext auf dem echten Windows
   Build noch nicht gegengeprüft (Sitzung 4, 2026-08-30).** Der komplette
   Ablauf inklusive Escaping eines Windows artigen Pfads mit
   Laufwerksbuchstaben wurde an einem lokalen Linux FFmpeg erfolgreich
   durchgetestet (siehe Abschnitt "KI Kennzeichnung, Schritt 5" oben),
   nicht aber am tatsächlichen, über die Baupipeline besorgten Windows
   FFmpeg. Sollte beim ersten echten Baulauf mit einem kurzen Testvideo
   einmal durchlaufen werden.

## Kompilierstatus

Anders als bei NovaPhonic (siehe dortige `BAUANLEITUNG.md`, dort ohne
Rust Compiler von Hand geschrieben und geprüft) stand in dieser Sitzung
erstmals eine echte Rust Toolchain zur Verfügung. Sowohl `cargo check` als
auch ein vollständiger `cargo build` liefen am 2026-08-29 fehlerfrei durch,
ebenso `cargo clippy` (nur einige stilistische Hinweise, keine Fehler,
keine Warnungen zu falschem Verhalten). Der gesamte Rust Code unter
`tauri-app/src-tauri/src/` ist damit erstmals nachweislich kompilierbar,
nicht nur sorgfältig von Hand geprüft.

Wichtige Einschränkung: dieser Testbau lief in einer Linux Umgebung ohne
die echten Sidecar Programme. Tauris Build System verlangt für
`externalBin` lediglich, dass zum Host Zielsystem passende Platzhalter
Dateien vorhanden sind, ihr tatsächlicher Inhalt spielt für `cargo
check`/`cargo build` keine Rolle. Für einen echten, lauffähigen Build auf
einem Windows Rechner werden weiterhin die drei echten Windows
Programmdateien benötigt (`auto-editor-x86_64-pc-windows-msvc.exe`,
`deep-filter-x86_64-pc-windows-msvc.exe`,
`ffmpeg-x86_64-pc-windows-msvc.exe`), genau nach dem Muster, das
NovaPhonics `BAUANLEITUNG.md` bereits beschreibt. Der erste Build mit den
echten Sidecars auf einem Windows Rechner oder per GitHub Actions bleibt
also weiterhin der erste vollständige Praxistest der Anwendung als
Ganzes.
