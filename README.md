# NovaStudioCast

Ein Tool von KI-WERKE. Schneidet mehrere Videoclips automatisch zusammen, entfernt
Füllwörter und Stille, gleicht Lautstärke sowie Klangbalance an und rendert daraus
einen einzigen fertigen Film mit weichen Übergängen zwischen den Clips, ein
selbstgebautes, lokales Gegenstück zu einem einfachen Videoschnittprogramm.

> **Läuft komplett lokal.** NovaStudioCast arbeitet vollständig auf dem eigenen
> Rechner, keine Internetverbindung während der Verarbeitung nötig, keine
> Anmeldung, keine Kosten, keine Videodaten verlassen den eigenen Rechner.

> **In aktiver Entwicklung.** Es gibt noch keinen veröffentlichten Installer. Wer
> sehen will, was als Nächstes kommt, oder mitreden möchte: die
> [KIW Schmiede Community auf Skool](https://www.skool.com/kiw-schmiede-9100)

## Installation

Sobald der erste automatische Windows Build über GitHub Actions erfolgreich
gelaufen ist, erscheint hier ein Link zur Releases Seite mit der fertigen
`setup.exe`. Bis dahin steht nur der Quellcode zur Verfügung, siehe
`tauri-app/BAUANLEITUNG.md` für den lokalen Aufbau.

Nur für Windows geplant. auto-editor, DeepFilterNet, FFmpeg sowie eine portable
Node.js Laufzeit mit Remotion für das finale Rendern werden bereits jetzt so
gebündelt, dass am Ende nichts zusätzlich von Hand installiert werden muss.

## Nutzung

1. Mehrere Videoclips per Drag and Drop in das Fenster ziehen, oder über die
   Dateiauswahl laden.
2. Reihenfolge der Clips bei Bedarf per Drag and Drop anpassen.
3. Bei Bedarf die Einstellungen anpassen: Mindestabstand um Sprechpausen
   (Margin), Entrauschen ein oder aus, Lautstärkeanpassung ein oder aus und
   Zielwert (LUFS), Länge der Übergänge zwischen den Clips.
4. Auf Start klicken. Jeder Clip wird einzeln geschnitten, entrauscht und in der
   Lautstärke angepasst, mit Fortschrittsanzeige pro Clip. Anschließend werden
   alle Clips mit weichen Übergängen zu einem einzigen Film gerendert.
5. Ergebnis über "Speichern unter" an den gewünschten Ort auf der Festplatte
   legen.

## Technische Details

NovaStudioCast ist eine schlanke Tauri Oberfläche, die im Hintergrund mehrere
eigenständige Open Source Werkzeuge als sogenannte Sidecar Programme aufruft:

- **auto-editor** (Public Domain, Unlicense): schneidet Füllwörter und Stille
  anhand von Lautstärke aus jedem Clip.
- **DeepFilterNet / deep-filter** (MIT/Apache 2.0): entfernt Hintergrundgeräusche
  aus der Tonspur.
- **FFmpeg**: gleicht Lautstärke und Klangbalance an (`loudnorm`, EBU R128) und
  übernimmt Audio Extraktion sowie das Wiederzusammenfügen von Bild- und Tonspur.
- **Node.js mit Remotion** (portable Laufzeit, ebenfalls als Sidecar gebündelt):
  fügt die geschnittenen Clips mit weichen Überblendungen zu einem einzigen
  fertigen Film zusammen.

Alle Werkzeuge laufen ohne Grafikkarte auf einer normalen CPU. Die Videodateien
verlassen nie den eigenen Rechner, es findet keine Verbindung zu einem Server
statt.

## Grenzen

- Reine Lautstärke basierte Schnitterkennung, keine inhaltliche Prüfung.
  Schnitte innerhalb eines Clips können vereinzelt etwas abrupt wirken.
- Entrauschen verbessert Hintergrundgeräusche, ersetzt aber keine professionelle
  Studioaufnahme oder ein bezahltes Tonstudio.
- Aktuell nur für Aufnahmen mit einer einzelnen sprechenden Person je Clip
  gedacht. Bei mehreren Personen im selben Clip werden Stimmen nicht getrennt
  erkannt.
- Noch kein veröffentlichter Installer, noch keine automatische
  Update-Prüfung.

## Nächste Schritte (geplant)

- Erster veröffentlichter Windows Installer über GitHub Actions.
- Chat KI Funktion, Cloud/API Stufe mit Bezahlschranke (lokale Stufe mit
  Ollama ist bereits umgesetzt und getestet).
- Untertitel, Kapitelmarken und Kennzeichnung KI-bearbeiteter Inhalte gemäß
  EU AI Act.

## Lizenz

GNU General Public License v3.0 (GPLv3), siehe [LICENSE](LICENSE). Wer den Code
verändert und weitergibt, muss den veränderten Quellcode ebenfalls unter der
GPLv3 offenlegen, das gilt auch bei kommerzieller Nutzung.

## Markenrechte

Die Namen "NovaStudioCast", "KI-WERKE" und "KIW Schmiede" sowie die
zugehörigen Logos sind eigenständig geschützte Kennzeichen von KI-WERKE und
ausdrücklich nicht Teil der GPLv3-Lizenz. Der Quellcode darf verändert und
weitergegeben werden, die Namen und Logos dürfen dabei aber nicht für eigene,
insbesondere abgewandelte oder umbenannte Versionen verwendet werden. Details
siehe [LICENSE](LICENSE).
