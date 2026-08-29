# Bauanleitung: NovaStudioCast Windows Installer

Dieses Verzeichnis enthält das Tauri Projekt, das NovaStudioCast als eigenständige
Windows Anwendung mit NSIS Installer (.exe) verpackt, nach demselben Muster wie
NovaPhonic. Der Installer selbst kann nicht in der Cloud Sandbox gebaut werden
(Linux, kein Rust, keine Windows Build Werkzeuge, keine Root Rechte), sondern nur
auf einem Windows Rechner oder per GitHub Actions.

Anders als NovaPhonic hat `tauri-app/` hier keine eigene `package.json` im
Wurzelverzeichnis (das Frontend unter `app/` ist reines HTML/CSS/JS ohne Bundler).
Gebaut wird deshalb mit der über Cargo installierten Tauri CLI (`cargo tauri
build`), nicht mit `npm run tauri build`. Eine eigene `package.json` gibt es nur
unter `tauri-app/remotion/`, für das separate Remotion Projekt, das die
geschnittenen Clips am Ende zu einem Film zusammenfügt.

## Wichtiger Hinweis zum Rust Code

In der Umgebung, in der dieses Projekt erstellt wurde, gab es keinen Rust
Compiler zum echten Ausprobieren auf Windows. `cargo check`, `cargo build` und
`cargo clippy` liefen unter Linux erfolgreich durch, der erste Build auf einem
echten Windows Rechner (lokal oder per GitHub Actions) ist trotzdem der erste
echte Test unter den Zielbedingungen. Bei einem Fehlschlag bitte die
Fehlermeldung schicken, dann wird gezielt nachgebessert.

## Schritt 1: Sidecar Werkzeuge besorgen

NovaStudioCast bringt vier Sidecar Programme direkt im Installer mit, damit
niemand sie selbst von Hand herunterladen und im PATH einrichten muss:
auto-editor, DeepFilterNet (`deep-filter`), FFmpeg sowie eine portable Node.js
Laufzeit für Remotion. Dafür müssen die vier Dateien vor dem Bauen einmalig in
`tauri-app/src-tauri/binaries/` liegen, mit dem Ziel-Tripel im Namen (auf einem
normalen Windows PC ist das `x86_64-pc-windows-msvc`, zur Sicherheit mit
`rustc -vV` prüfen, Zeile "host"):

```
tauri-app/src-tauri/binaries/auto-editor-x86_64-pc-windows-msvc.exe
tauri-app/src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
tauri-app/src-tauri/binaries/deep-filter-x86_64-pc-windows-msvc.exe
tauri-app/src-tauri/binaries/node-x86_64-pc-windows-msvc.exe
```

Diese PowerShell Befehle laden die ersten drei automatisch herunter und
benennen sie richtig (im Ordner `tauri-app` ausführen):

```powershell
New-Item -ItemType Directory -Force -Path "src-tauri\binaries" | Out-Null

# auto-editor
Invoke-WebRequest -Uri "https://github.com/WyattBlue/auto-editor/releases/latest/download/auto-editor-windows-x86_64.exe" `
  -OutFile "src-tauri\binaries\auto-editor-x86_64-pc-windows-msvc.exe"

# FFmpeg (gyan.dev essentials build, gepackt als Zip)
Invoke-WebRequest -Uri "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile "ffmpeg.zip"
Expand-Archive -Path "ffmpeg.zip" -DestinationPath "ffmpeg-extracted"
$ffmpegExe = Get-ChildItem -Path "ffmpeg-extracted" -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
Copy-Item $ffmpegExe.FullName "src-tauri\binaries\ffmpeg-x86_64-pc-windows-msvc.exe"
Remove-Item "ffmpeg.zip", "ffmpeg-extracted" -Recurse -Force

# Node.js (offizielle, portable Windows Version, Version 20 LTS)
Invoke-WebRequest -Uri "https://nodejs.org/dist/latest-iron/win-x64/node.exe" `
  -OutFile "src-tauri\binaries\node-x86_64-pc-windows-msvc.exe"

# DeepFilterNet: Release Seite oeffnen und die aktuelle Windows Datei manuell laden,
# da sich der genaue Dateiname von Version zu Version aendern kann:
# https://github.com/Rikorose/DeepFilterNet/releases/latest
# Danach die heruntergeladene Datei umbenennen/kopieren nach:
# src-tauri\binaries\deep-filter-x86_64-pc-windows-msvc.exe
```

Die GitHub Actions Pipeline (`.github/workflows/build.yml`) erledigt genau diese
vier Downloads bereits automatisch bei jedem Cloud Build, inklusive einer
automatischen Suche nach der aktuellen DeepFilterNet Windows Datei. Für einen
lokalen Build auf dem eigenen Rechner müssen die Dateien wie oben einmalig von
Hand besorgt werden.

## Schritt 2: Remotion Projekt vorbereiten

Zusätzlich zu den vier Sidecar Dateien braucht das Remotion Projekt unter
`tauri-app/remotion/` einmalig seine Node Abhängigkeiten sowie den dafür
nötigen headless Chromium:

```
cd tauri-app/remotion
npm install
node -e "require('@remotion/renderer').ensureBrowser().then(() => console.log('Chromium bereit.'))"
```

Der zweite Befehl lädt den zu Windows passenden Chromium einmalig herunter und
legt ihn unter `node_modules/.remotion/` ab, von wo aus er zusammen mit dem
Rest von `node_modules` über `tauri.conf.json` (`bundle.resources`) mit in den
Installer gepackt wird. Ohne diesen Schritt bräuchte der Nutzer beim
allerersten eigenen Rendern eine Internetverbindung, genau das soll
"vollständig lokal" ja gerade vermeiden.

## Weg 1: Auf dem eigenen Windows Rechner bauen

Einmalige Voraussetzungen:

1. Node.js (LTS Version): https://nodejs.org
2. Rust über https://rustup.rs
3. Während/nach der Rust Installation: "Microsoft C++ Build Tools" (Komponente
   "Desktop development with C++" aus dem Visual Studio Installer).
4. WebView2 Runtime ist auf aktuellen Windows 10/11 Systemen normalerweise schon
   dabei.
5. Die vier Sidecar Dateien aus Schritt 1 liegen in `src-tauri/binaries/`.
6. Schritt 2 (Remotion Projekt vorbereiten) wurde ausgeführt.

```
cargo install tauri-cli --version "^2.0.0" --locked
cd tauri-app/src-tauri
cargo tauri build
```

Installer liegt danach unter:

```
tauri-app\src-tauri\target\release\bundle\nsis\NovaStudioCast_<Version>_x64-setup.exe
```

Zum Testen ohne Installer, mit Live Neuladen: `cargo tauri dev` (im selben
Ordner `tauri-app/src-tauri`).

## Weg 2: Automatisch in der Cloud bauen (GitHub Actions)

Ist bereits eingerichtet unter `.github/workflows/build.yml`, inklusive dem
automatischen Herunterladen der vier Sidecar Werkzeuge sowie der Installation
und Chromium Vorbereitung des Remotion Projekts. Läuft automatisch bei jedem
Push, der `tauri-app/` oder `app/` verändert, und kann jederzeit manuell über
"Run workflow" im Actions Tab gestartet werden. Ergebnis liegt dort als
Artifact. Dauert spürbar länger als bei NovaPhonic, weil zusätzlich Node
Abhängigkeiten installiert und Chromium heruntergeladen werden.

## Ein echtes Release veröffentlichen (mit Versionsnummer)

Ohne Versions-Tag baut die Cloud nur ein Artifact, das nach 30 Tagen automatisch
verschwindet. Für ein dauerhaftes, versioniertes GitHub Release:

1. Versionsnummer an zwei Stellen anheben (muss gleich sein), z.B. von 0.1.0 auf
   0.2.0:
   - `tauri-app/src-tauri/Cargo.toml` -> `version = "..."`
   - `tauri-app/src-tauri/tauri.conf.json` -> `"version"`
2. Änderungen committen wie gewohnt.
3. Einen Versions-Tag setzen und pushen:

```
git tag v0.2.0
git push origin v0.2.0
```

4. Der Tag-Push löst den Workflow erneut aus, baut den Installer und legt diesmal
   zusätzlich automatisch ein GitHub Release mit dem Namen des Tags an, der NSIS
   Installer (.exe) hängt als Download-Datei direkt am Release, dauerhaft, nicht
   nur 30 Tage.

## Wie Updates beim Nutzer ankommen

Genau wie bei NovaPhonic prüft NovaStudioCast beim Start automatisch, ob eine
neue Version auf GitHub Releases liegt. Voraussetzung, damit das funktioniert:

1. Ein eigenes Signierschlüsselpaar erzeugen (Minisign):

```
npx @tauri-apps/cli signer generate
```

2. Den ausgegebenen öffentlichen Schlüssel in
   `tauri-app/src-tauri/tauri.conf.json` unter `plugins.updater.pubkey`
   eintragen, anstelle des Platzhaltertexts.
3. Den privaten Schlüssel als GitHub Actions Secrets hinterlegen (Repository ->
   Settings -> Secrets and variables -> Actions -> "New repository secret"):
   - `TAURI_SIGNING_PRIVATE_KEY`: Inhalt der privaten Schlüsseldatei.
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Passwort des Schlüssels (leer
     lassen, falls ohne Passwort erzeugt, das Secret dann mit leerem Wert
     anlegen).

Ohne diese beiden Secrets schlägt der Release-Build bei einem Versions-Tag
fehl.

## Deinstallieren

Ganz normal wie jedes andere Windows Programm, über Windows Einstellungen ->
Apps -> Installierte Apps -> "NovaStudioCast" suchen -> Deinstallieren.

## Warum überhaupt eine eigene App

NovaStudioCast bündelt vier separate Open Source Werkzeuge (auto-editor,
DeepFilterNet, FFmpeg, Node.js mit Remotion) hinter einer einzigen, einfachen
Bedienoberfläche mit Fortschrittsanzeige, sodass niemand PowerShell Befehle
von Hand eintippen muss.
