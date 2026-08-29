# NovaStudioCast, Projektgedächtnis

Diese Datei fasst den aktuellen Stand, die wichtigsten Entscheidungen und
die offenen Punkte des Projekts zusammen. Gedacht als erster Einstiegspunkt
für jede neue Arbeitssitzung, ausführlichere technische Details stehen in
`docs/ARCHITEKTUR.md` und `docs/JSON_SCHEMA.md`.

## Projektziel

Vollständig lokale, ressourcenschonende Pipeline für automatisierte
Videoproduktion, in Tauri und Rust, als Weiterentwicklung von NovaPhonic
(`C:\KIW-SCHMIEDE\NOVA-PHONIC`). Vier Schritte, strikt sequenziell, für
einen Batch aus einem oder mehreren Videos:

1. Entrauschen (DeepFilterNet)
2. Audioveredelung, Equalizer und EBU R128 Lautheit (FFmpeg)
3. Schnittanalyse, nur Erkennung, kein Rendering (Auto Editor)
4. Zusammenfügen und finales Rendering (Remotion)

Bedienung zweigeteilt: links ein Chatbereich als Steuerelement (Drag and
Drop der Videos, Textbefehle über eine lokale Sprachmodell Anbindung),
rechts ein dynamischer Kontextbereich mit Reglern, Fortschritt, Vorschau
und einer per Maus sortierbaren Reihenfolge Liste für die finale
Zeitleiste. Optik soll exakt an NovaPhonic angeglichen werden.

## Stand nach dem ersten Entwicklungsauftrag (2026-08-19)

Erstellt wurde die Grundstruktur des Rust Controllers unter
`tauri-app/src-tauri/src/`, siehe `docs/ARCHITEKTUR.md` für die genaue
Modulaufteilung. Die Schritte 1 und 2 sind fachlich vollständig aus
NovaPhonic übernommen (Sidecar Aufrufe, FFmpeg Filterketten,
DeepFilterNet Parameter, unverändert). Schritt 3 ist neu konzipiert:
Auto Editor exportiert nur noch eine JSON Schnittliste
(`--export v1`), rendert kein Video mehr. Das exakte JSON Format
zwischen Schritt 3 und Schritt 4 ist in `docs/JSON_SCHEMA.md` definiert,
in drei Ebenen: Auto Editors Rohformat, die normalisierte
NovaStudioCast Schnittliste pro Clip, und das Gesamt Manifest für
Remotion.

Noch nicht Teil dieses Auftrags: die Remotion Anbindung selbst
(`pipeline/render.rs` ist ein dokumentierter Platzhalter), das Frontend,
sowie `tauri.conf.json`/Capabilities/Icons.

## Wichtige Entscheidungen und Abweichungen vom Briefing

- **Reihenfolge geändert auf Entrauschen, dann Schnittanalyse, dann
  Audioveredelung (1, 3, 2 statt 1, 2, 3 aus dem ursprünglichen
  Briefing).** Am 2026-08-19 gemeinsam mit JJ besprochen und
  entschieden (JJ hatte keine Präferenz zwischen den vorgeschlagenen
  Varianten, deshalb wurde die empfohlene Lösung umgesetzt): Auto Editor
  analysiert jetzt standardmäßig die Datei direkt nach DeepFilterNet
  (`AnalyzeSource::PostDenoise`, neuer Standardwert, vorher hieß die
  Variante `Processed` und meinte die komplett veredelte Datei nach
  Loudnorm), also noch vor Equalizer, Kompressor und
  Lautheitsnormierung. War Entrauschen deaktiviert, läuft die Analyse
  automatisch auf der Originaldatei, ganz ohne Sonderfall, weil
  `ctx.current` dann unverändert auf sie zeigt. Grund für die Reihenfolge:
  NovaPhonic hat am 2026-08-19 gelernt, dass Auto Editor mit einem festen
  Lautstärke Schwellwert arbeitet und bei durchgehendem
  Hintergrundgeräusch im Originalton oft nichts als Stille erkennt, das
  Entrauschen vorher behebt das zuverlässig. Da weder Entrauschen noch
  Lautheitsnormierung die Bildspur verändern (`-c:v copy`), passen die
  erkannten Zeitstempel unabhängig davon auf jede der drei Dateifassungen
  (Original, entrauscht, komplett veredelt). Wer trotzdem das Verhalten
  aus dem ursprünglichen Briefing möchte (immer die Originaldatei
  analysieren, auch bei aktivem Entrauschen), stellt
  `analyzeSource: "original"` in den PipelineOptions ein.
- Gilt automatisch auch im Batch: die neue Reihenfolge greift pro Clip
  innerhalb von `run_clip_steps`, an der äußeren Batch Schleife
  (`run_batch`, strikt nacheinander über alle Clips) ändert sich dadurch
  nichts.
- **Warnung bei fehlender Stille wird eingebaut.** Entscheidung von JJ am
  2026-08-19. Findet Auto Editor bei einem Clip gar keine Stille (die
  normalisierte Schnittliste enthält dann nur ein einziges `keepSegment`,
  das die gesamte Clipdauer abdeckt), soll `cut_analysis.rs` dafür eine
  auffällige Warnung ausgeben (`emit_progress` mit eigenem Status, z.B.
  `"warning"`, damit die Oberfläche das optisch von normalen
  Fortschrittsmeldungen unterscheiden kann), statt das stillschweigend
  durchlaufen zu lassen. Wichtig gerade im Batch, damit ein Ausreißer
  unter vielen Clips nicht übersehen wird. **Noch nicht umgesetzt, siehe
  offene Punkte unten.**

## Stand der Umsetzung dieser Entscheidung, 2026-08-19 Abend

Bereits fertig umgesetzt und in dieser Sitzung in den Projektordner
geschrieben:

- `pipeline/types.rs`: `AnalyzeSource::Processed` in
  `AnalyzeSource::PostDenoise` umbenannt, Standardwert und
  Dokumentationskommentare aktualisiert.
- `pipeline/mod.rs`: `run_clip_steps` umgebaut, führt jetzt Schritt 1
  (Entrauschen), dann Schritt 3 (Schnittanalyse), dann Schritt 2
  (Audioveredelung) aus, mit aktualisierten Kommentaren.

**Noch offen, bitte als Erstes bei der Fortsetzung erledigen, rein
kosmetisch, keine Funktionsänderung mehr nötig:**

- `pipeline/cut_analysis.rs`: Kopfkommentar verweist noch auf die alte
  Formulierung ("JJ entscheidet", "Processed" als Variantenname), sollte
  auf die jetzt getroffene Entscheidung und den neuen Variantennamen
  `PostDenoise` angepasst werden.
- `pipeline/audio_refine.rs`: Kopfkommentar sagt noch, der Schritt laufe
  "bewusst VOR der Schnittanalyse", das stimmt nicht mehr, muss auf "läuft
  jetzt NACH der Schnittanalyse" korrigiert werden.
- `docs/ARCHITEKTUR.md`: Abschnitt "Was sich gegenüber NovaPhonic ändert"
  und der bisherige offene Punkt zum `analyzeSource` Standardwert müssen
  auf die getroffene Entscheidung aktualisiert werden, der Punkt gilt
  damit als erledigt.
- `docs/JSON_SCHEMA.md`: Beschreibung des Felds `analyzedSource` in Ebene
  2 sagt noch "bereits entrauschte und lautheitsnormierte Fassung", muss
  auf "bereits entrauschte, aber noch nicht lautheitsnormierte Fassung"
  präzisiert werden.

Ab hier weiterarbeiten, sobald JJ grünes Licht gibt (angekündigt für
"morgen oder übermorgen", Stand 2026-08-19).

## Offene Punkte für die nächste Sitzung

Stand nach Sitzung 3 (2026-08-29), siehe dortigen Abschnitt weiter unten
für alles bereits Erledigte:

1. **Chatbereich mit echter Funktion.** Aktuell nur eine stumme optische
   Hülle (siehe Sitzung 3), JJ hat das bewusst so für heute freigegeben.
   Eine lokale Sprachmodell Anbindung, die Textbefehle in echte Tauri
   Befehle übersetzt, ist ein eigenes, noch nicht begonnenes Vorhaben:
   Auswahl des lokalen Modells, Übersetzung von natürlicher Sprache in
   `run_batch_pipeline` Aufrufe mit den richtigen Parametern, Anbindung an
   die bestehende Videoliste und Einstellungen.
2. Echte, NovaStudioCast eigene Icons statt der aktuellen NovaPhonic
   Platzhalter Icons.
3. Das GitHub Repository einmalig anlegen und pushen, siehe
   `GITHUB_SETUP.md`, danach den ersten echten Lauf der Baupipeline
   beobachten und etwaige Fehler beheben (am wahrscheinlichsten beim
   DeepFilterNet oder beim neuen Chromium Download Schritt, siehe dortige
   Fehlerhinweise). Die Sidecar Programme, node.exe, sowie npm install und
   der Windows Chromium für Remotion werden dabei automatisch besorgt,
   dafür ist kein manueller Schritt mehr nötig.
4. Beim ersten erfolgreichen Baulauf prüfen:
   Flagname `--export v1` in der installierten Auto Editor Version, sowie
   generell alle mit "prüfen" oder "TODO" markierten Stellen im Code.
5. PipelineOptions auf das `Option<...Options>` Muster umbauen, siehe
   Sitzung 2, bisher bewusst zurückgestellt.
6. Danach die in Sitzung 2 entschiedenen neuen Funktionen umsetzen:
   Untertitel (whisper.cpp), Kapitelmarken (Hybrid Modell), KI Kennzeichnung
   nach Artikel 50 EU AI Act.

## Sitzung 2 (2026-08-26), neue Funktionen besprochen: Untertitel, KI Kennzeichnung, Kapitelmarken

Reine Planungssitzung, ausdrücklich noch kein Code geschrieben, JJ wollte
erst die Architektur klären. Alle hier festgehaltenen Punkte gelten als
Arbeitsgrundlage für die nächste Umsetzungssitzung, zusätzlich zu den
bereits weiter oben offenen Punkten aus Sitzung 1.

### Entscheidung: Umbau der PipelineOptions

Wegen der wachsenden Zahl an unabhängig an und abschaltbaren Funktionen
(Entrauschen, Audioveredelung, jetzt zusätzlich Untertitel, KI
Kennzeichnung, Kapitelmarken) wird `PipelineOptions` umgebaut: jede
optionale Funktion bekommt ihre Einstellungen in einem eigenen Baustein,
der entweder ganz fehlt (Funktion aus) oder mit eigenen Werten vorhanden
ist (Funktion an), statt einzelner loser Felder wie bisher bei `denoise`
und `loudnorm`. JJ hat dazu keine ausdrückliche Präferenz genannt, ist
aber mit weiteren Funktionen fortgefahren, was den Vorschlag zusätzlich
stützt. Gilt als Arbeitsgrundlage, sollte vor den neuen Funktionen als
Erstes umgesetzt werden.

### Untertitel, Entscheidungen getroffen

- Zeitpunkt: eigener, fünfter Schritt NACH dem fertigen Gesamtvideo aus
  Remotion, nicht vorher pro Einzelclip. Keine Umrechnung durch die
  Schnittliste nötig, sauber an und abschaltbar, ohne die Schritte 1 bis 4
  zu berühren.
- Werkzeug für die lokale, kostenlose Stufe: whisper.cpp, als weiteres
  natives Sidecar Programm wie auto-editor, ffmpeg und deep-filter, läuft
  ohne Python oder GPU auf der CPU. JJ hat bestätigt, dass für den
  lokalen Rechner ein leichtgewichtiges Werkzeug richtig ist.
  Zukunftshinweis von JJ: für eine spätere kostenpflichtige Serverstufe
  (dieselbe, die schon für die WhisperX/Pyannote Mehrsprechererkennung
  vorgesehen ist, siehe `NOVA-PHONIC_Setup_WhisperX_ROCm.md`) soll
  perspektivisch das dortige, genauere Modell auch für Untertitel
  genutzt werden können. Architektonische Konsequenz: die
  Transkriptions-Engine sollte hinter einer austauschbaren Stelle
  liegen (z.B. eine Funktion, die Audio entgegennimmt und ein
  einheitliches Segment/Wort Zeitstempel Format zurückgibt), damit
  whisper.cpp später ohne Umbau der restlichen Untertitel Logik (SRT
  Erzeugung, Einbettung) gegen das Server Modell getauscht werden kann.
- Ausgabeform: sowohl als eigenständige `.srt` Datei neben dem fertigen
  Video als auch zusätzlich als eingebettete, zuschaltbare
  Untertitelspur direkt im Video (verlustfreier ffmpeg Remux mit `-c
  copy`, kein erneutes Kodieren).

### Neuer Punkt: KI Kennzeichnung nach Artikel 50 EU AI Act

Von JJ eingebracht, ausdrücklich für ganz am Ende einplanbar, kein
aktueller Blocker. Gedacht als eigenes, per Haken an und abschaltbares
Modul, das eine Kennzeichnung ("KI generiert", "KI überarbeitet" oder
ähnlich) sichtbar in das fertige Video einbrennt. Zwei mögliche Wege,
noch nicht entschieden:

1. Eigenes kleines NovaStudioCast Modul, das den Hinweistext direkt per
   FFmpeg oder Remotion als Bildeinblendung setzt.
2. Weiterreichen an NovaImage (das andere bestehende KI WERKE Werkzeug):
   NovaStudioCast müsste dafür selbst nichts bauen, der Nutzer müsste
   aber zusätzlich NovaImage installiert haben. Noch zu klären, ob
   NovaImage sich für diesen Zweck (Text/Wasserzeichen auf Video statt
   nur auf Bildern) überhaupt eignet, das wurde in dieser Sitzung nicht
   geprüft.

Rechtlicher Hinweis, keine rechtliche Beratung: ob und in welcher Form
Artikel 50 EU AI Act auf die konkreten Ausgaben von NovaStudioCast
zutrifft (reine Schnitt und Klangbearbeitung versus z.B. später
hinzukommende KI generierte Inhalte), ist eine juristische Einschätzung,
die im Zweifel fachkundig geprüft werden sollte, bevor die Kennzeichnung
scharf geschaltet oder bewusst weggelassen wird.

### Kapitelmarken, finale Entscheidung: Hybrid aus automatischem Vorschlag und freier manueller Setzung

JJ hat das Bild in zwei Schritten geschärft. Zuerst: es reicht nicht,
ein Kapitel starr pro hochgeladenem Clip anzunehmen, er will echte,
freie Zeitmarken setzen können, genau wie man das direkt in YouTube
selbst tun kann, auch innerhalb eines einzelnen, langen Clips.
Kapiteltitel werden vom Nutzer selbst eingegeben. Danach ergänzt: jeder
Clipwechsel, den Remotion auf der Zeitleiste vornimmt, markiert in der
Praxis fast immer ohnehin einen inhaltlichen Themenwechsel (das ist laut
JJ "so ein bisschen Sinn und Zweck von Remotion"), deshalb sollen
Clipgrenzen als sinnvolle Startvorschläge automatisch mitgeliefert
werden. Zusätzlich soll der Nutzer im fertigen Video ganz einfach klicken
können: "da soll eine Kapitelmarke hin, da soll eine hin, da soll eine
hin."

Daraus ergibt sich folgendes, jetzt festgelegtes Modell:

- **Automatischer Vorschlag:** beim Öffnen der fertigen Video Vorschau
  wird für jeden Clipwechsel auf der finalen Zeitleiste (aus Remotions
  tatsächlichen Positionen, siehe unten) bereits eine Kapitelmarke
  vorbelegt, mit einem Platzhaltertitel (z.B. Dateiname des jeweiligen
  Clips oder schlicht "Kapitel 2", "Kapitel 3"). Das erste Kapitel bei
  0:00 ergibt sich dabei automatisch aus dem Start des ersten Clips.
- **Freie manuelle Bearbeitung:** der Nutzer kann in der Oberfläche
  (Scrubber im rechten Kontextbereich, ähnlich YouTube Studio) jede
  automatisch vorgeschlagene Marke umbenennen oder löschen, und
  zusätzlich an jeder beliebigen Stelle im fertigen Video eigene neue
  Marken setzen, unabhängig von Clipgrenzen. Datenmodell weiterhin
  denkbar einfach: eine geordnete Liste aus Zeitpunkt (Sekunden) und
  Titel, egal ob automatisch vorgeschlagen oder vom Nutzer manuell
  hinzugefügt.
- Kapitel werden erst auf dem fertigen Video aus Schritt 4 gesetzt bzw.
  vorgeschlagen, nicht vorher, damit sowohl die automatischen Vorschläge
  als auch alles manuell Gesetzte exakt zur tatsächlich gerenderten
  Zeitleiste passt (Überblendungen aus `transitionSeconds` verschieben
  sonst die exakte Position). Reiht sich damit sauber neben Untertitel
  als weiterer, rein optionaler Nachbearbeitungsschritt nach dem
  Rendering ein, betrifft nicht die Schritte 1 bis 4. Der Scrubber
  selbst sowie die automatische Vorbelegung der Clipgrenzen Vorschläge
  sind in erster Linie eine Frontend Aufgabe.
- **Alle Plattformen bedienen, ausdrücklicher Auftrag von JJ:** beide
  Ausgabeformen werden gebaut, nicht nur eine.
  1. Ein fertig formatierter Textblock zum Einfügen in die YouTube
     Videobeschreibung, da YouTube Kapitel ausschließlich so erkennt
     (erstes Kapitel zwingend bei 0:00, ein Eintrag pro Zeile, Zeit
     gefolgt vom Titel, mindestens drei Kapitel mit je mindestens 10
     Sekunden Länge, sonst ignoriert YouTube die ganze Liste). YouTube
     liest KEINE in die Videodatei eingebetteten Metadaten.
  2. Zusätzlich echte, in die MP4 Datei eingebettete Kapitel Metadaten
     (per ffmpeg Chapter Format, verlustfreies Remuxen, kein erneutes
     Kodieren), damit Videoplayer und Schnittprogramme (VLC, Editing
     Software, etc.) die Kapitel direkt aus der Datei selbst lesen
     können, ohne YouTube. Kann im selben verlustfreien Remux Durchgang
     wie das Einbetten der Untertitelspur erledigt werden.

## Sitzung 3 (2026-08-29), Kompiliertest, Remotion Anbindung, Grundgerüst

JJ hat die aus Sitzung 2 offenen Punkte in dieser Reihenfolge freigegeben
("Let's do it"): kosmetische Korrekturen, Warnung bei fehlender Stille,
Remotion Architekturentscheidung, das eigentliche Remotion Projekt, das
Frontend, sowie `tauri.conf.json` samt Capabilities und Icons.

**Erledigt:**

- Die vier kosmetischen Korrekturen aus Sitzung 1/2 (Kommentare in
  `cut_analysis.rs` und `audio_refine.rs`, sowie `docs/ARCHITEKTUR.md` und
  `docs/JSON_SCHEMA.md`) sind vollständig umgesetzt.
- Die Warnung bei fehlender Stille ist vollständig umgesetzt:
  `cutlist::likely_no_silence_found` (ein einziges `keepSegment` über
  mindestens 98% der Clipdauer), Warnmeldung per `emit_progress` mit
  Status `"warning"` in `cut_analysis.rs`, Sammelfeld `warnings` im
  Gesamt Manifest (`manifest.rs`, `BatchWarning`), dokumentiert in
  `docs/JSON_SCHEMA.md`.
- **Erstmaliger echter Kompiliertest.** Diese Sitzung hatte erstmals
  Zugriff auf eine echte Rust Toolchain. Nach Installation fehlender
  Tauri Linux Systempakete liefen `cargo check`, ein vollständiger
  `cargo build` sowie `cargo clippy` fehlerfrei durch. Dafür musste ein
  minimales Grundgerüst angelegt werden, das vorher fehlte: `build.rs`
  (wortgleich aus NovaPhonic), Icons (vorerst NovaPhonic Platzhalter),
  ein Platzhalter `app/index.html`, sowie neu `tauri.conf.json` und
  `capabilities/default.json`. Details siehe `docs/ARCHITEKTUR.md`,
  Abschnitt "Kompilierstatus".
- **Remotion Architekturentscheidung getroffen und umgesetzt.** Beim
  Testen kam heraus, dass Remotion zusätzlich zu Node.js immer einen
  headless Chromium Browser braucht (separater Download,
  `node_modules/.remotion/`), und dass Remotions Lizenz ab mehr als 3
  Mitarbeitenden kostenpflichtig wird. Beides wurde JJ mitgeteilt, bevor
  weitergebaut wurde. JJ Entscheidung: wie geplant mit Remotion
  weitermachen, Weg a) (portable Node.js Laufzeit als Sidecar plus
  gebündeltes Remotion Projekt als Ressource), da KI-WERKE ein
  Einzelunternehmen ohne Angestellte ist ("Ich bin eine One-Man-Show,
  ich habe keine 3 Angestellten, werde ich auch nie haben."), die
  Lizenzfrage damit unkritisch ist. Vollständig dokumentiert in
  `docs/ARCHITEKTUR.md`.
- **Echtes Remotion Projekt aufgesetzt unter `tauri-app/remotion/`:**
  Komposition "novastudiocast-timeline" (`src/Root.tsx`,
  `src/Timeline.tsx`, Überblendungen per `@remotion/transitions`), sowie
  `render.mjs` als Renderskript, das Manifest und Schnittlisten liest,
  einen lokalen HTTP Server für die Quelldateien startet (Remotion braucht
  das, da Kompositionen im Browser Kontext laufen und keinen direkten
  Dateisystemzugriff auf beliebige absolute Pfade haben) und
  rendert. **Vollständig lokal getestet:** zwei Testclips per ffmpeg
  erzeugt, ein Beispiel Manifest von Hand nach dem Schema aus
  `docs/JSON_SCHEMA.md` geschrieben, `render.mjs` direkt ausgeführt,
  Ergebnis war ein korrektes MP4 mit exakt der erwarteten Dauer und
  Auflösung. Damit ist Schritt 4 nicht nur entworfen, sondern nachweislich
  lauffähig.
- `pipeline/render.rs` ist entsprechend nicht mehr nur ein Platzhalter,
  sondern ruft `render.mjs` über den `node` Sidecar auf, genau wie die
  anderen drei Schritte über `run_sidecar`.
- `tauri.conf.json`, `capabilities/default.json` und `main.rs`'s
  `SIDECARS` Liste um den `node` Sidecar sowie `bundle.resources` für den
  Remotion Projektordner erweitert.
- `.gitignore` neu angelegt (Rust `target/`, Remotion `node_modules/`,
  Sidecar Binärdateien), damit diese großen, lokal neu erzeugbaren Ordner
  nicht versehentlich mit ins Projekt oder auf das Gerät übertragen
  werden.

**Noch offen:** echte NovaStudioCast Icons. Alles andere aus diesem Absatz
(Windows Sidecar Binärdateien inklusive `node.exe`, Windows Chromium
Cache) läuft inzwischen automatisch über die GitHub Actions Baupipeline,
siehe direkt unten.

### Windows Installer per GitHub Actions (Fortsetzung Sitzung 3)

JJ hat gefragt, ob schon eine echte `setup.exe` existiert. Klargestellt:
nein, bis dahin gab es nur nachweislich kompilierenden Quellcode, keinen
fertigen Installer. Zusätzlich richtig gestellt: `npm install` für
Remotion soll nicht beim Nutzer laufen, sondern nur einmal beim Bauen,
das fertige Ergebnis wird direkt mit in die `setup.exe` gepackt, der
Nutzer sieht davon nichts.

Diese Entwicklungsumgebung kann selbst keinen geprüft funktionierenden
Windows Installer erzeugen (Linux ohne Testmöglichkeit auf echtem Windows,
zusätzlich eingeschränkter Zugriff auf github.com aus dieser Umgebung
heraus, GitHub Downloads einzelner Programme funktionieren von hier aus
nicht zuverlässig). Zur Wahl gestellt: GitHub Actions Pipeline einrichten,
selbst eine Bauanleitung für JJs eigenen Windows Rechner schreiben, oder
trotz der Einschränkungen hier einen ungetesteten Versuch wagen. **JJ hat
sich für GitHub Actions entschieden.**

Umgesetzt: `.github/workflows/build.yml` wurde direkt von NovaPhonics
eigenem, bereits produktiv laufendem Workflow übernommen (dort bereits
bewährt, lädt automatisch auto-editor, DeepFilterNet und FFmpeg für
Windows herunter und baut den Installer) und um zwei Dinge ergänzt: einen
Schritt, der die Node Abhängigkeiten des Remotion Projekts installiert und
den passenden Windows Chromium vorab herunterlädt, sowie einen vierten
Sidecar Download für `node.exe` (offizielle Node.js Distribution). Details
und die technische Begründung stehen in `docs/ARCHITEKTUR.md`. Eine
`GITHUB_SETUP.md` mit der genauen Schritt für Schritt Anleitung zum
Anlegen und Pushen des Repositories liegt ebenfalls bereit, JJ muss das
Repository einmalig selbst auf GitHub anlegen und den ersten Push machen,
da diese Sitzung keinen GitHub Zugriff hat.

**Ehrlich benannt:** der Workflow wurde sorgfältig aus einem bereits
laufenden Vorbild abgeleitet, konnte aber mangels Windows Umgebung und
GitHub Zugriff hier nicht selbst ausgeführt werden. Der erste Lauf nach
dem Einrichten des Repositories ist der erste echte Test.

### Frontend gebaut (Fortsetzung Sitzung 3)

Auf Nachfrage entschieden: JJ wollte den Chatbereich als stumme optische
Hülle bereits mitgebaut haben (nicht funktionslos weggelassen, aber auch
keine echte lokale Sprachmodell Anbindung heute), siehe Antwortoption
"Chat als stumme Optik Hülle mitbauen".

- Design eins zu eins von NovaPhonics echtem Frontend übernommen (Farben,
  Schriften Space Grotesk/Inter/JetBrains Mono, Logo, Dark/Light Umschalter),
  Schriften und Logo Dateien direkt aus `C:\KIW-SCHMIEDE\NOVA-PHONIC\app\assets`
  übernommen, keine Nachbauten.
- Layout wie im ursprünglichen Briefing zweigeteilt: links der Chatbereich
  (`app/index.html`, `.chat-panel`, deutlich mit Badge "Demnächst"
  gekennzeichnet, Eingabefeld bewusst deaktiviert, damit niemand versehentlich
  eine nicht angeschlossene Funktion für kaputt hält), rechts der
  dynamische Kontextbereich mit Einstellungen, der Videoliste samt
  Reihenfolge (Drag and Drop, klassisches HTML Drag and Drop, keine
  Zusatzbibliothek), Fortschrittsanzeige je Clip plus einer eigenen Zeile
  für den batchweiten Remotion Schritt, einer eigenen Warnungen Box für
  die in Sitzung 3 eingebaute "keine Stille gefunden" Meldung, und dem
  Ergebnisbereich mit "Speichern unter".
- `app.js` spricht ausschließlich die tatsächlichen Tauri Befehle und
  Feldnamen aus `main.rs`/`pipeline/types.rs` an (`pick_video_files`,
  `run_batch_pipeline` mit `jobs`/`options` genau nach `ClipJob` und
  `PipelineOptions`, `pipeline-progress` Ereignis nach `ProgressPayload`).
- **Echt getestet, nicht nur entworfen:** die fertige Oberfläche wurde per
  Playwright in einem echten Chromium geladen (ohne Tauri, das ist ohne
  eine echte Windows Umgebung nicht möglich), mit einem simulierten
  `window.__TAURI__` durchgespielt: Videos hinzufügen, Reihenfolge und
  Dateigrößen korrekt anzeigen, einzelnen Clip entfernen, kompletten
  simulierten Fortschritt inklusive der Warnung und des abschließenden
  Render Schritts durchlaufen lassen, Ergebnisbereich mit "Speichern
  unter" Knopf. Keine JavaScript Fehler, alle Zustände sahen wie erwartet
  aus, sowohl im Dark als auch im Light Modus.
- Nicht enthalten, bewusst: Werte für das noch nicht in der Oberfläche
  wählbare Feld `analyzeSource` (wird fest auf `postDenoise` gesetzt,
  siehe getroffene Entscheidung aus Sitzung 1), sowie jegliche echte Chat
  Funktion.

## Referenzen

- NovaPhonic Quellcode und Dokumentation: `C:\KIW-SCHMIEDE\NOVA-PHONIC`
  (insbesondere `tauri-app/src-tauri/src/main.rs` als fachliche Basis für
  Schritt 1 und 2, sowie `app/style.css` als Vorlage für die Optik).
- Auto Editor v1 Exportformat, offizielle Dokumentation:
  https://auto-editor.com/docs/v1
- Auto Editor v3 Exportformat (vollständiges Timeline Format, aktuell nicht
  verwendet, aber als Referenz relevant, falls Remotion später mehr als nur
  Behalten/Wegschneiden Abschnitte braucht, z.B. Effekte): https://auto-editor.com/docs/v3
- `C:\KIW-SCHMIEDE\NOVA-PHONIC\NOVA-PHONIC_Setup_WhisperX_ROCm.md` betrifft
  die separat geplante, kostenpflichtige Mehrsprechererkennung (Stufe 2 von
  NovaPhonic) und wurde für diesen Auftrag nur zur Kenntnis genommen, nicht
  inhaltlich ausgewertet.
