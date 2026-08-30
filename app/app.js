/*
 * app.js
 * NovaStudioCast Oberfläche. Ruft ausschließlich Tauri Rust Commands auf,
 * die im Hintergrund auto-editor, DeepFilterNet, FFmpeg und Remotion (über
 * den node Sidecar) als eigenständige Programme ausführen. Videodateien
 * selbst werden nie in dieses Skript geladen, es werden nur Dateipfade
 * zwischen Oberfläche und Rust ausgetauscht.
 *
 * Der linke Chatbereich (siehe index.html, .chat-panel) ist bewusst nur
 * eine optische Hülle ohne echte Funktion. Eine Steuerung per Textbefehl
 * über eine lokale Sprachmodell Anbindung ist ein eigenes, größeres
 * Vorhaben (Wahl des Modells, Übersetzung von Text in echte Tauri
 * Befehle), siehe MEMORY.md, "Offene Punkte für die nächste Sitzung". Bis
 * dahin läuft die gesamte Steuerung über den rechten Kontextbereich.
 */
(function () {
  "use strict";

  function $(id) { return document.getElementById(id); }
  const root = document.documentElement;

  function isTauri() {
    return typeof window.__TAURI__ !== 'undefined' && window.__TAURI__.core;
  }

  // ---------- Theme ----------
  const THEME_KEY = 'novastudiocast-theme';
  const themeToggle = $('themeToggle');
  const themeLabel = $('themeLabel');

  function applyTheme(theme) {
    root.setAttribute('data-theme', theme);
    themeLabel.textContent = theme === 'dark' ? 'Dunkel' : 'Hell';
    try { localStorage.setItem(THEME_KEY, theme); } catch (e) { /* ignore */ }
  }
  (function initTheme() {
    let saved = null;
    try { saved = localStorage.getItem(THEME_KEY); } catch (e) { /* ignore */ }
    applyTheme(saved === 'light' ? 'light' : 'dark');
  })();
  themeToggle.addEventListener('click', () => {
    const current = root.getAttribute('data-theme');
    applyTheme(current === 'dark' ? 'light' : 'dark');
  });

  // ---------- Werkzeug Check ----------
  async function checkTools() {
    if (!isTauri()) return;
    try {
      const missing = await window.__TAURI__.core.invoke('check_tools');
      if (Array.isArray(missing) && missing.length > 0) {
        $('toolWarning').style.display = 'block';
        $('toolWarningText').textContent =
          'Nicht gefunden: ' + missing.join(', ') +
          '. Bitte in der BAUANLEITUNG.md nachsehen, wie diese Werkzeuge bereitgestellt werden.';
      }
    } catch (e) {
      // check_tools selbst nicht verfügbar, z.B. während der Entwicklung im Browser
    }
  }

  function formatBytes(bytes) {
    if (!bytes || bytes <= 0) return '';
    const units = ['B', 'KB', 'MB', 'GB'];
    let i = 0, val = bytes;
    while (val >= 1024 && i < units.length - 1) { val /= 1024; i++; }
    return val.toFixed(val >= 10 || i === 0 ? 0 : 1) + ' ' + units[i];
  }

  function fileNameOf(path) {
    return path.split(/[\\/]/).pop();
  }

  // ---------- Videoauswahl und Reihenfolge Liste ----------
  // Jeder Clip bekommt eine im Batch eindeutige, frei gewählte Kennung
  // (clip-1, clip-2, …), unabhängig von seiner Position in der Liste. Die
  // Position in `clips` selbst bestimmt beim Start die `order`, die an den
  // Rust Controller übergeben wird, siehe pipeline/types.rs, ClipJob.
  let clips = [];
  let nextClipNumber = 1;
  let dragFromIndex = null;

  function addClip(path, sizeBytes) {
    if (clips.some((c) => c.path === path)) return;
    clips.push({ id: 'clip-' + (nextClipNumber++), path, sizeBytes: sizeBytes || 0 });
    renderClipList();
  }

  function removeClip(index) {
    clips.splice(index, 1);
    renderClipList();
  }

  function reorderClip(fromIndex, toIndex) {
    if (fromIndex === toIndex) return;
    const [moved] = clips.splice(fromIndex, 1);
    clips.splice(toIndex, 0, moved);
    renderClipList();
  }

  function renderClipList() {
    const list = $('clipList');
    list.innerHTML = '';
    clips.forEach((clip, index) => {
      const li = document.createElement('div');
      li.className = 'clip-item';
      li.draggable = true;
      li.dataset.index = String(index);

      li.innerHTML =
        '<span class="ci-handle" aria-hidden="true">' +
        '<svg viewBox="0 0 24 24"><path d="M8 6h.01M8 12h.01M8 18h.01M16 6h.01M16 12h.01M16 18h.01"/></svg>' +
        '</span>' +
        '<span class="ci-order">' + (index + 1) + '</span>' +
        '<span class="ci-name">' + fileNameOf(clip.path).replace(/</g, '&lt;') + '</span>' +
        '<span class="ci-size">' + formatBytes(clip.sizeBytes) + '</span>' +
        '<button class="ci-remove" type="button" aria-label="Aus dem Batch entfernen">' +
        '<svg viewBox="0 0 24 24"><path d="M18 6L6 18M6 6l12 12"/></svg></button>';

      li.querySelector('.ci-remove').addEventListener('click', (e) => {
        e.stopPropagation();
        removeClip(index);
      });

      li.addEventListener('dragstart', () => {
        dragFromIndex = index;
        li.classList.add('dragging');
      });
      li.addEventListener('dragend', () => {
        dragFromIndex = null;
        li.classList.remove('dragging');
        list.querySelectorAll('.clip-item').forEach((el) => el.classList.remove('drop-target'));
      });
      li.addEventListener('dragover', (e) => {
        e.preventDefault();
        li.classList.add('drop-target');
      });
      li.addEventListener('dragleave', () => li.classList.remove('drop-target'));
      li.addEventListener('drop', (e) => {
        e.preventDefault();
        li.classList.remove('drop-target');
        if (dragFromIndex !== null) reorderClip(dragFromIndex, index);
      });

      list.appendChild(li);
    });

    $('clipCountTag').textContent = clips.length > 0
      ? clips.length + (clips.length === 1 ? ' Video' : ' Videos')
      : '';
    $('startBtn').disabled = clips.length === 0;
  }

  async function pickFiles() {
    if (!isTauri()) {
      alert('Dateiauswahl ist nur innerhalb der NovaStudioCast Anwendung verfügbar, nicht im Browser.');
      return;
    }
    try {
      const results = await window.__TAURI__.core.invoke('pick_video_files');
      for (const result of results) {
        if (result && result.path) addClip(result.path, result.sizeBytes);
      }
    } catch (e) {
      console.error(e);
    }
  }

  const dropzone = $('dropzone');
  dropzone.addEventListener('click', pickFiles);
  dropzone.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickFiles(); }
  });

  // Echtes Drag & Drop von Dateien aus dem Windows Explorer in das Fenster,
  // mehrere Dateien auf einmal möglich. Liefert direkt Dateipfade, es wird
  // nichts im Browser eingelesen.
  (function initDragDrop() {
    if (!isTauri() || !window.__TAURI__.webview) return;
    try {
      window.__TAURI__.webview.getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === 'over') {
          dropzone.classList.add('drag');
        } else if (event.payload.type === 'drop') {
          dropzone.classList.remove('drag');
          const paths = event.payload.paths || [];
          const validExt = ['mp4', 'mov', 'mkv', 'avi', 'webm'];
          let rejected = false;
          for (const p of paths) {
            const ext = p.split('.').pop().toLowerCase();
            if (validExt.includes(ext)) {
              addClip(p, null);
            } else {
              rejected = true;
            }
          }
          if (rejected) alert('Nur Videodateien wurden übernommen (MP4, MOV, MKV, AVI oder WEBM).');
        } else {
          dropzone.classList.remove('drag');
        }
      });
    } catch (e) {
      // Drag & Drop API in dieser Tauri Version evtl. anders benannt, kein Problem,
      // Klicken zum Auswählen funktioniert trotzdem.
      console.warn('Drag & Drop nicht verfügbar:', e);
    }
  })();

  // ---------- Einstellungen ----------
  const marginSlider = $('marginSlider');
  const marginTag = $('marginTag');
  function updateMarginTag() {
    const val = (parseInt(marginSlider.value, 10) / 10).toFixed(1);
    marginTag.textContent = val.replace('.', ',') + ' s';
  }
  marginSlider.addEventListener('input', updateMarginTag);
  updateMarginTag();

  const loudnormToggle = $('loudnormToggle');
  const loudnormOptions = $('loudnormOptions');
  function syncLoudnormOptions() {
    loudnormOptions.style.display = loudnormToggle.checked ? 'block' : 'none';
  }
  loudnormToggle.addEventListener('change', syncLoudnormOptions);
  syncLoudnormOptions();

  // KI Kennzeichnung nach Artikel 50 EU AI Act (Sitzung 4, 2026-08-30).
  // Siehe pipeline/ai_disclosure.rs und pipeline/types.rs für die
  // Rust seitige Umsetzung. Standardmäßig aus, weil es sich um ein rein
  // optionales, per Haken zuschaltbares Compliance Modul handelt, siehe
  // MEMORY.md.
  const aiDisclosureToggle = $('aiDisclosureToggle');
  const aiDisclosureOptionsBox = $('aiDisclosureOptions');
  function syncAiDisclosureOptions() {
    aiDisclosureOptionsBox.style.display = aiDisclosureToggle.checked ? 'block' : 'none';
  }
  aiDisclosureToggle.addEventListener('change', syncAiDisclosureOptions);
  syncAiDisclosureOptions();

  const aiDisclosureText = $('aiDisclosureText');
  const aiDisclosureCustomTextRow = $('aiDisclosureCustomTextRow');
  function syncAiDisclosureCustomText() {
    aiDisclosureCustomTextRow.style.display = aiDisclosureText.value === 'custom' ? 'block' : 'none';
  }
  aiDisclosureText.addEventListener('change', syncAiDisclosureCustomText);
  syncAiDisclosureCustomText();

  const aiDisclosureDurationMode = $('aiDisclosureDurationMode');
  const aiDisclosureDurationSeconds = $('aiDisclosureDurationSeconds');
  function syncAiDisclosureDuration() {
    aiDisclosureDurationSeconds.style.display = aiDisclosureDurationMode.value === 'fixed' ? 'inline-block' : 'none';
  }
  aiDisclosureDurationMode.addEventListener('change', syncAiDisclosureDuration);
  syncAiDisclosureDuration();

  // Baut das aiDisclosure Feld für den Tauri Aufruf, `null` wenn der
  // Baustein ausgeschaltet ist (siehe PipelineOptions::ai_disclosure,
  // Option<...> in Rust, deserialisiert `null` als `None`).
  function buildAiDisclosureOptions() {
    if (!aiDisclosureToggle.checked) return null;
    const textChoice = aiDisclosureText.value;
    const durationMode = aiDisclosureDurationMode.value;
    const startOffset = parseFloat($('aiDisclosureStartOffset').value);
    const fixedDuration = parseFloat(aiDisclosureDurationSeconds.value);
    return {
      text: textChoice,
      customText: textChoice === 'custom' ? $('aiDisclosureCustomText').value : null,
      position: $('aiDisclosurePosition').value,
      timing: {
        startReference: $('aiDisclosureStartReference').value,
        startOffsetSeconds: Number.isFinite(startOffset) ? startOffset : 0,
        durationSeconds: durationMode === 'fixed'
          ? (Number.isFinite(fixedDuration) ? fixedDuration : null)
          : null,
      },
    };
  }

  // ---------- Pipeline Ablauf ----------
  // Je Clip drei Schritte (siehe pipeline/mod.rs, run_clip_steps), plus ein
  // batchweiter Render Schritt am Ende (clipId ist dabei null, siehe
  // pipeline/types.rs, ProgressPayload). Fortschrittsgruppen werden bei
  // Bedarf dynamisch angelegt, sobald die erste Meldung für einen Clip
  // eintrifft, nicht schon beim Start.
  const CLIP_STEPS = [
    { id: 'denoise', label: 'Rauschen entfernen' },
    { id: 'cut-analysis', label: 'Füllwörter & Stille erkennen' },
    { id: 'loudnorm', label: 'Klang veredeln & Lautstärke angleichen' },
  ];

  function clipNameById(clipId) {
    const clip = clips.find((c) => c.id === clipId);
    return clip ? fileNameOf(clip.path) : clipId;
  }

  function ensureClipProgressGroup(clipId) {
    const areaId = 'progress-group-' + clipId;
    let group = $(areaId);
    if (group) return group;

    $('progressEmptyHint').style.display = 'none';

    group = document.createElement('div');
    group.className = 'clip-progress-group';
    group.id = areaId;
    group.innerHTML =
      '<div class="cpg-title">' + clipNameById(clipId).replace(/</g, '&lt;') + '</div>' +
      '<div class="pipeline-steps">' +
      CLIP_STEPS.map((s) =>
        '<div class="pipeline-step" id="step-' + clipId + '-' + s.id + '">' +
        '<span class="ps-dot"></span>' +
        '<span class="ps-label">' + s.label + '</span>' +
        '<span class="ps-detail" id="step-' + clipId + '-' + s.id + '-detail"></span>' +
        '</div>'
      ).join('') +
      '</div>';
    $('progressArea').appendChild(group);
    return group;
  }

  function ensureRenderProgressRow() {
    let row = $('step-render');
    if (row) return row;
    $('progressEmptyHint').style.display = 'none';

    const group = document.createElement('div');
    group.className = 'clip-progress-group';
    group.innerHTML =
      '<div class="cpg-title">Gesamtes Video (Remotion)</div>' +
      '<div class="pipeline-steps">' +
      '<div class="pipeline-step" id="step-render">' +
      '<span class="ps-dot"></span>' +
      '<span class="ps-label">Zusammenfügen &amp; Rendern</span>' +
      '<span class="ps-detail" id="step-render-detail"></span>' +
      '</div></div>';
    $('progressArea').appendChild(group);
    return $('step-render');
  }

  function ensureAiDisclosureProgressRow() {
    let row = $('step-ai-disclosure');
    if (row) return row;
    $('progressEmptyHint').style.display = 'none';

    const group = document.createElement('div');
    group.className = 'clip-progress-group';
    group.innerHTML =
      '<div class="cpg-title">KI-Kennzeichnung (Artikel 50 EU AI Act)</div>' +
      '<div class="pipeline-steps">' +
      '<div class="pipeline-step" id="step-ai-disclosure">' +
      '<span class="ps-dot"></span>' +
      '<span class="ps-label">Hinweis einblenden</span>' +
      '<span class="ps-detail" id="step-ai-disclosure-detail"></span>' +
      '</div></div>';
    $('progressArea').appendChild(group);
    return $('step-ai-disclosure');
  }

  function resetProgress() {
    $('progressArea').innerHTML = '<div class="empty-hint" id="progressEmptyHint">Verarbeitung läuft…</div>';
    $('logBox').textContent = '';
    $('logBox').style.display = 'none';
    $('warningsPanel').style.display = 'none';
    $('warningsBox').innerHTML = '';
  }

  function appendLog(line) {
    const box = $('logBox');
    box.style.display = 'block';
    box.textContent += (box.textContent ? '\n' : '') + line;
    box.scrollTop = box.scrollHeight;
  }

  function addWarning(clipId, message) {
    $('warningsPanel').style.display = 'block';
    const item = document.createElement('div');
    item.className = 'warning-item';
    item.innerHTML = '<span><strong>' + clipNameById(clipId).replace(/</g, '&lt;') +
      ':</strong> ' + String(message).replace(/</g, '&lt;') + '</span>';
    $('warningsBox').appendChild(item);
  }

  function setStepStatus(clipId, step, status, detail) {
    let el;
    if (clipId) {
      ensureClipProgressGroup(clipId);
      el = $('step-' + clipId + '-' + step);
    } else if (step === 'ai-disclosure') {
      el = ensureAiDisclosureProgressRow();
    } else {
      el = ensureRenderProgressRow();
    }
    if (!el) return;
    el.classList.remove('running', 'done', 'error', 'warning');
    if (status) el.classList.add(status === 'warning' ? 'warning' : status);
    const detailId = el.id + '-detail';
    if (detail !== undefined) {
      const detailEl = $(detailId);
      if (detailEl) detailEl.textContent = detail || '';
    }
  }

  let unlistenProgress = null;

  async function initProgressListener() {
    if (!isTauri()) return;
    unlistenProgress = await window.__TAURI__.event.listen('pipeline-progress', (event) => {
      const p = event.payload || {};
      if (p.step) setStepStatus(p.clipId, p.step, p.status, p.detail);
      if (p.status === 'warning' && p.detail) addWarning(p.clipId, p.detail);
      if (p.log) appendLog((p.clipId ? clipNameById(p.clipId) + ': ' : '') + p.log);
    });
  }

  const startBtn = $('startBtn');
  startBtn.addEventListener('click', async () => {
    if (clips.length === 0 || !isTauri()) return;
    startBtn.disabled = true;
    dropzone.style.pointerEvents = 'none';
    resetProgress();
    $('resultArea').innerHTML = '<div class="empty-hint">Verarbeitung läuft…</div>';

    const jobs = clips.map((clip, index) => ({
      id: clip.id,
      inputPath: clip.path,
      order: index,
    }));
    const options = {
      denoise: $('denoiseToggle').checked,
      loudnorm: loudnormToggle.checked,
      loudnormTarget: parseInt($('loudnormTarget').value, 10),
      marginSeconds: parseInt(marginSlider.value, 10) / 10,
      analyzeSource: 'postDenoise',
      aiDisclosure: buildAiDisclosureOptions(),
    };

    try {
      const result = await window.__TAURI__.core.invoke('run_batch_pipeline', { jobs, options });
      showResult(result);
    } catch (e) {
      console.error(e);
      $('resultArea').innerHTML =
        '<div class="status-error">Verarbeitung fehlgeschlagen: ' +
        String(e).replace(/</g, '&lt;') + '</div>';
    } finally {
      startBtn.disabled = clips.length === 0;
      dropzone.style.pointerEvents = '';
    }
  });

  function showResult(result) {
    const name = fileNameOf(result.finalVideoPath);
    $('resultArea').innerHTML =
      '<div class="result-box">' +
      '<span class="rb-name">' + name.replace(/</g, '&lt;') + '</span>' +
      '<button id="saveResultBtn" type="button">Speichern unter…</button>' +
      '</div>' +
      '<div class="status-ok" style="margin-top:8px;">Fertig. Liegt vorerst unter: ' +
      String(result.finalVideoPath).replace(/</g, '&lt;') + '</div>';
    $('saveResultBtn').addEventListener('click', async () => {
      try {
        const saved = await window.__TAURI__.core.invoke('save_output_as', {
          sourcePath: result.finalVideoPath,
          suggestedName: name,
        });
        if (saved) appendLog('Gespeichert unter: ' + saved);
      } catch (e) {
        console.error(e);
      }
    });
  }

  // ---------- Updater (wie bei NovaPhonic) ----------
  const updateBanner = $('updateBanner');
  const updateBannerText = $('updateBannerText');
  let pendingUpdate = null;

  $('updateDismissBtn').addEventListener('click', () => {
    updateBanner.style.display = 'none';
  });
  $('updateInstallBtn').addEventListener('click', async () => {
    if (!pendingUpdate) return;
    updateBannerText.innerHTML = '<strong>Wird installiert…</strong>';
    try {
      await pendingUpdate.downloadAndInstall();
      await window.__TAURI__.process.relaunch();
    } catch (e) {
      console.error(e);
      updateBannerText.innerHTML = '<strong>Update fehlgeschlagen.</strong> Bitte später erneut versuchen oder den Installer manuell von GitHub laden.';
    }
  });

  async function checkForUpdate(silent) {
    if (!isTauri() || !window.__TAURI__.updater) return 'none';
    try {
      const update = await window.__TAURI__.updater.check();
      if (update) {
        pendingUpdate = update;
        updateBannerText.innerHTML =
          '<strong>Update verfügbar (' + update.version + ').</strong> Eine neue Version von NovaStudioCast steht bereit.';
        updateBanner.style.display = 'flex';
        return 'available';
      }
      return 'none';
    } catch (e) {
      if (!silent) console.error(e);
      return 'error';
    }
  }

  const checkUpdateBtn = $('checkUpdateBtn');
  if (isTauri() && window.__TAURI__.updater) {
    checkUpdateBtn.style.display = 'inline-flex';
  }
  checkUpdateBtn.addEventListener('click', async () => {
    checkUpdateBtn.disabled = true;
    checkUpdateBtn.textContent = 'Prüfe…';
    const status = await checkForUpdate(false);
    if (status === 'available') {
      checkUpdateBtn.disabled = false;
      checkUpdateBtn.textContent = 'Nach Updates suchen';
    } else if (status === 'error') {
      checkUpdateBtn.textContent = 'Prüfung fehlgeschlagen';
      setTimeout(() => {
        checkUpdateBtn.disabled = false;
        checkUpdateBtn.textContent = 'Nach Updates suchen';
      }, 2500);
    } else {
      checkUpdateBtn.textContent = 'Bereits aktuell';
      setTimeout(() => {
        checkUpdateBtn.disabled = false;
        checkUpdateBtn.textContent = 'Nach Updates suchen';
      }, 2500);
    }
  });

  // ---------- Start ----------
  renderClipList();
  initProgressListener();
  checkTools();
  checkForUpdate(true);
})();
