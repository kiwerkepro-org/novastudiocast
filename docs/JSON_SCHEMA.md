# NovaStudioCast, JSON Datenformate zwischen Schritt 3 und Schritt 4

Dieses Dokument beschreibt exakt, welche JSON Dateien der Rust Controller in
Schritt 3 (Schnittanalyse) erzeugt und was Remotion in Schritt 4 daraus
liest. Es gibt drei Ebenen, die auseinandergehalten werden müssen.

## Ebene 1, Auto Editors eigenes Rohformat ("v1")

Auto Editor selbst schreibt bei `--export v1` eine JSON Datei in seinem
offiziellen, öffentlich dokumentierten Format
(https://auto-editor.com/docs/v1). NovaStudioCast liest diese Datei ein und
verarbeitet sie sofort weiter, sie wird nicht direkt an Remotion
weitergegeben.

```json
{
  "version": "1",
  "source": "C:/KIW-SCHMIEDE/eingabe/clip01.mp4",
  "timebase": "30000/1001",
  "chunks": [
    [0, 145, 1],
    [145, 210, 99999],
    [210, 900, 1]
  ]
}
```

Feldbedeutung:

- `timebase`: Bruch als Text, z.B. `"30000/1001"` für 29,97 fps. Alle
  Zahlen in `chunks` sind Vielfache dieser Einheit, also Frames, nicht
  Sekunden.
- `chunks`: Liste aus Dreiergruppen `[start, end, speed]`. `start` ist
  einschließlich, `end` ausschließlich, beide in Timebase Einheiten. Der
  erste Eintrag beginnt immer bei 0, jeder folgende Eintrag beginnt genau
  dort, wo der vorherige endet, Lücken sind nicht erlaubt.
- `speed`: `1.0` bedeutet, der Abschnitt bleibt unverändert erhalten.
  Jeder andere Wert, in der Praxis meist `99999.0`, markiert einen von
  Auto Editor als Stille bzw. Füllwort erkannten Abschnitt, der
  weggeschnitten werden soll.

Im Beispiel oben: Frame 0 bis 145 behalten, Frame 145 bis 210 wegschneiden,
Frame 210 bis 900 wieder behalten.

## Ebene 2, die normalisierte NovaStudioCast Schnittliste pro Clip

Der Rust Controller wandelt die Ebene 1 Rohdatei direkt nach dem Auto
Editor Aufruf in ein eigenes, sekundenbasiertes Format um (Modul
`cutlist.rs`, Funktion `normalize`) und schreibt sie als eigene Datei
`<Dateiname>.cuts.json` neben die veredelte Videodatei des Clips.

Warum eine eigene Zwischenschicht statt Auto Editors Rohdaten direkt an
Remotion zu geben: erstens rechnet Auto Editor in Frame Einheiten der
jeweiligen Quelldatei, während Remotion Kompositionen üblicherweise in
Sekunden oder der eigenen Projekt fps arbeiten, eine Umrechnung muss also
ohnehin irgendwo passieren. Zweitens bleibt Remotion dadurch unabhängig von
Auto Editors Exportformat, eine spätere Änderung an Auto Editor betrifft
dann nur `cutlist.rs`, nicht die Remotion Seite.

```json
{
  "schemaVersion": "1.0",
  "clipId": "clip01",
  "order": 0,
  "analyzedSource": "C:/Users/.../Temp/novastudiocast-1234/clip-clip01/clip01_nova.mp4",
  "fps": 29.97002997002997,
  "durationSeconds": 30.0,
  "keepSegments": [
    { "startSeconds": 0.0, "endSeconds": 4.838333333333333 },
    { "startSeconds": 7.0, "endSeconds": 30.0 }
  ]
}
```

Feldbedeutung:

- `clipId`: dieselbe Kennung, die das Frontend beim Ablegen der Datei im
  Chat vergeben hat, eindeutig innerhalb eines Batch Laufs.
- `order`: Position auf der finalen Zeitleiste, vom Nutzer per Drag and
  Drop in der Reihenfolge Liste festgelegt, 0 basiert.
- `analyzedSource`: Pfad der tatsächlich analysierten Datei. Zeigt im
  Regelfall auf die Ausgabe von Schritt 1, also die bereits entrauschte,
  aber noch NICHT lautheitsnormierte oder mit Equalizer bearbeitete
  Fassung, nicht auf die ursprüngliche Eingabedatei, siehe Begründung in
  `pipeline/cut_analysis.rs`.
- `fps`: aus Auto Editors `timebase` errechnet (Zähler geteilt durch
  Nenner), reine Fließkommazahl, keine Bruchschreibweise mehr.
- `durationSeconds`: Gesamtlänge der analysierten Datei in Sekunden, aus dem
  Ende des letzten Auto Editor Chunks abgeleitet.
- `keepSegments`: geordnete Liste der zu behaltenden Abschnitte in
  Sekunden, jeweils vom Beginn der analysierten Datei aus gezählt.
  Unmittelbar aneinandergrenzende Behalten Abschnitte sind bereits
  zusammengefasst. Alles, was zwischen zwei Segmenten liegt, gilt als
  weggeschnitten und darf von Remotion nicht mit auf die Zeitleiste
  übernommen werden.

## Ebene 3, das Gesamt Manifest für Remotion

Nach Abschluss der Schritte 1 bis 3 für ALLE Clips im Batch schreibt der
Rust Controller genau einmal ein Gesamt Manifest (Modul `manifest.rs`),
sortiert nach der vom Nutzer festgelegten Zeitleisten Reihenfolge. Dieses
Manifest, nicht die einzelnen `cuts.json` Dateien, ist die eigentliche
Schnittstelle zu Remotion in Schritt 4, es wird als `--props` Datei an den
Remotion Render Aufruf übergeben.

```json
{
  "schemaVersion": "1.0",
  "projectId": "novastudiocast-8842",
  "generatedAt": "2026-08-19T20:14:03+00:00",
  "transitionSeconds": 0.15,
  "warnings": [],
  "timeline": [
    {
      "order": 0,
      "clipId": "clip01",
      "processedVideoPath": "C:/Users/.../Temp/novastudiocast-8842/clip-clip01/clip01_nova.mp4",
      "cutListPath": "C:/Users/.../Temp/novastudiocast-8842/clip-clip01/clip01.cuts.json",
      "fps": 29.97002997002997,
      "durationSeconds": 30.0
    },
    {
      "order": 1,
      "clipId": "clip02",
      "processedVideoPath": "C:/Users/.../Temp/novastudiocast-8842/clip-clip02/clip02_nova.mp4",
      "cutListPath": "C:/Users/.../Temp/novastudiocast-8842/clip-clip02/clip02.cuts.json",
      "fps": 29.97002997002997,
      "durationSeconds": 18.4
    }
  ]
}
```

Feldbedeutung:

- `transitionSeconds`: weiche Überblendung, die Remotion an jeder
  Clipgrenze und an jedem Schnittpunkt anwenden soll. Aus `marginSeconds`
  der Pipeline Einstellungen abgeleitet, ersetzt das `--transition` Flag,
  das NovaPhonic bisher direkt an auto-editor übergeben hat, weil
  auto-editor in NovaStudioCast in Schritt 3 nicht mehr selbst rendert.
- `timeline`: Liste in der finalen Abspielreihenfolge. Remotion liest pro
  Eintrag die zugehörige `cutListPath` Datei (Ebene 2), platziert
  ausschließlich deren `keepSegments` aus `processedVideoPath`
  hintereinander auf der Hauptzeitleiste und wendet zwischen den
  Abschnitten die `transitionSeconds` Überblendung an. Am Ende entsteht
  daraus ein einziges finales Gesamtvideo.
- `warnings`: Sammelliste aller Clips im Batch, bei denen Auto Editor in
  Schritt 3 praktisch keine Stille gefunden hat (ein einziges
  `keepSegment` über nahezu die gesamte Clipdauer, siehe
  `cutlist::likely_no_silence_found`). Im Normalfall leer. Für Remotion
  ohne Bedeutung, gedacht für die Oberfläche, damit ein Ausreißer unter
  vielen Clips im Batch nicht übersehen wird.

Für die Remotion Seite bedeutet das konkret: die React Komposition erhält
dieses Manifest als `inputProps`, iteriert über `timeline` in der
angegebenen Reihenfolge, lädt für jeden Eintrag die referenzierte
`cutListPath` Datei nach und baut daraus die Sequence Elemente der
Zeitleiste, jeweils mit `startFrom`/`endAt`, umgerechnet aus den
`keepSegments` Sekundenwerten anhand der jeweiligen Clip `fps`.
