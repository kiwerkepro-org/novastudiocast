# NovaStudioCast auf GitHub einrichten

Ich habe hier keinen GitHub Zugriff (keine Zugangsdaten, kein verbundenes
GitHub Konto), das Repository muss deshalb einmalig von dir selbst angelegt
und gepusht werden, genau wie bei NovaPhonic. Der GitHub Actions Workflow,
der danach automatisch den Windows Installer baut, liegt bereits fertig
unter `.github/workflows/build.yml`.

Dieser Workflow übernimmt automatisch alles, was für eine funktionierende
setup.exe nötig ist, auf einem echten Windows Server bei GitHub:

- lädt die vier Sidecar Programme herunter (auto-editor, DeepFilterNet,
  FFmpeg, sowie neu die portable Node.js Laufzeit für Remotion),
- installiert die Node Abhängigkeiten des Remotion Projekts
  (`tauri-app/remotion`) per `npm ci`,
- lädt vorab den zu Windows passenden Chromium für Remotion herunter, damit
  er direkt mit in den Installer gepackt wird,
- baut die eigentliche Anwendung und packt alles zu einer einzigen
  `setup.exe` zusammen.

Für den Nutzer bleibt danach wirklich nur ein Doppelklick auf die
`setup.exe`. Kein npm, kein Node, kein manueller Zusatzschritt auf seinem
Rechner, alles davon ist bereits vorher beim Bauen passiert.

## Schritte

1. Auf GitHub ein neues, leeres Repository anlegen, zum Beispiel
   `https://github.com/kiwerkepro-org/novastudiocast` (passend zum
   Namensmuster von NovaPhonic und NovaImage, außerdem bereits genau die
   Adresse, die in `tauri-app/src-tauri/tauri.conf.json` als Update
   Endpunkt hinterlegt ist).

2. Eingabeaufforderung oder PowerShell öffnen und in den Ordner wechseln,
   in dem dieses NOVA-STUDIO-CAST Verzeichnis liegt, dann:

```
cd "C:\KIW-SCHMIEDE\NOVA-STUDIO-CAST" && git init && git add . && git commit -m "Initial commit: NovaStudioCast" && git branch -M main && git remote add origin https://github.com/kiwerkepro-org/novastudiocast.git && git push -u origin main
```

   Falls das Repository bereits eine README oder Lizenz enthält und der
   Push deswegen abgelehnt wird, hilft `git push -u origin main --force`
   (nur beim allerersten Push unbedenklich, da das Repo sonst leer ist),
   oder vorher `git pull origin main --allow-unrelated-histories`.

3. Danach im Repository auf GitHub den Reiter **Actions** öffnen. Der
   Workflow "Windows Installer bauen" startet automatisch nach dem Push
   (weil Dateien unter `tauri-app/` und `app/` enthalten sind) und kann
   zusätzlich jederzeit manuell über den Button **Run workflow** gestartet
   werden.

4. Nach erfolgreichem Lauf (dauert spürbar länger als bei NovaPhonic, weil
   zusätzlich Node Abhängigkeiten installiert und der Remotion Chromium
   heruntergeladen werden, insgesamt eher 10 bis 15 Minuten) findest du im
   jeweiligen Workflow Lauf unter **Artifacts** die Datei
   `novastudiocast-windows-installer` zum Herunterladen, darin enthalten
   der fertige `.exe` (NSIS) Installer.

## Falls der erste Cloud Build fehlschlägt

Am wahrscheinlichsten ist eines von zwei Dingen:

- Ein Problem beim automatischen Herunterladen der DeepFilterNet Windows
  Datei, weil sich deren genauer Dateiname von Version zu Version ändern
  kann (dasselbe Risiko besteht bereits bei NovaPhonic).
- Der neue Schritt "Chromium für Windows vorab herunterladen" schlägt
  fehl, weil sich an der `@remotion/renderer` API etwas geändert hat, oder
  weil der Download zu lange dauert und der Workflow Schritt in ein
  Timeout läuft.

Die Fehlermeldung im Actions Log zeigt in beiden Fällen klar an, woran es
liegt. Im Zweifel kurz die Log Ausgabe hierher kopieren, dann wird der
entsprechende Schritt im Workflow angepasst.

## Später: eigenes Signierschlüsselpaar für automatische Updates

Genau wie bei NovaPhonic wird die In App Aktualisierung erst funktionieren,
sobald ein eigenes Tauri Signierschlüsselpaar erzeugt und als Repository
Secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`)
hinterlegt wurde. Bis dahin baut der Workflow trotzdem eine ganz normal
installierbare `setup.exe`, nur die automatische Update Prüfung in der
Anwendung selbst läuft dann noch ins Leere.
