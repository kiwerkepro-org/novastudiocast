/*
 * app.js
 * NovaStudioCast Oberfläche. Ruft ausschließlich Tauri Rust Commands auf,
 * die im Hintergrund auto-editor, DeepFilterNet, FFmpeg und Remotion (über
 * den node Sidecar) als eigenständige Programme ausführen. Videodateien
 * selbst werden nie in dieses Skript geladen, es werden nur Dateipfade
 * zwischen Oberfläche und Rust ausgetauscht.
 *
 * Der linke Chatbereich (siehe index.html, .chat-panel) ist seit dieser
 * Sitzung echt angebunden: lokale Sprachmodell Steuerung über eine vom
 * Nutzer selbst installierte und laufende Ollama Instanz (siehe
 * src-tauri/src/chat/, HTTP auf localhost:11434). Der Chat kann
 * Einstellungen ändern und die Verarbeitung starten, siehe Abschnitt
 * "Chat / Ollama" weiter unten. Videos selbst kommen weiterhin
 * ausschließlich per Drag and Drop im rechten Bereich in die Liste.
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

  // ---------- Versionsanzeige ----------
  // "v0.1.0" in index.html war bisher ein fest eingetragener Platzhalter,
  // der nie die tatsächlich installierte Version widerspiegelt hat (JJ
  // Rückmeldung, Sitzung 4: "das ist ja nervig, ich will ja wissen, was
  // der Sache ist"). Liest die echte Version jetzt zur Laufzeit über die
  // Tauri App API aus (core:default deckt app:allow-version bereits ab,
  // keine Capabilities Änderung nötig). Läuft die Oberfläche mal ohne
  // Tauri (z.B. reine Browser Vorschau während der Entwicklung), bleibt
  // einfach der HTML Platzhalter stehen.
  async function initVersionTag() {
    if (!isTauri() || !window.__TAURI__.app) return;
    try {
      const version = await window.__TAURI__.app.getVersion();
      $('versionTag').textContent = 'v' + version;
    } catch (e) {
      console.warn('Version konnte nicht ausgelesen werden:', e);
    }
  }

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


  // ---------- Chat / Ollama ----------
  // Lokale Chat KI Steuerung ueber eine vom Nutzer selbst installierte und
  // laufende Ollama Instanz, siehe src-tauri/src/chat/ fuer die Rust
  // seitige HTTP Anbindung und Projektgedaechtnis "chat_ki_modelle" fuer
  // die Modellauswahl. Der Chat kann ausschliesslich Einstellungen aendern
  // und die Verarbeitung starten (siehe applyChatToolCall), Videos selbst
  // bleiben ausschliesslich per Drag and Drop im rechten Bereich steuerbar.
  //
  // Seit Sitzung 6: die Modellwahl/Einrichtung liegt in einem eigenen
  // Popup (chatSetupOverlay), nicht mehr fest im Chatbereich. Grund,
  // Rueckmeldung JJ: "wenn irgendetwas installiert ist, kann ich trotzdem
  // im Chat nichts eingeben", weil die Modellwahl vorher jedes Mal den
  // ganzen Chatbereich belegt hat, bis man sie durchgeklickt hatte. Chat
  // Nachrichten und Eingabefeld sind jetzt immer sichtbar, das "Lokal"
  // Abzeichen im Kopf ist immer klickbar und oeffnet bei Bedarf das Popup.
  const ACTIVE_CHAT_MODEL_KEY = 'novastudiocast-chat-model';

  const chatSetupOverlay = $('chatSetupOverlay');
  const chatSetupContent = $('chatSetupContent');
  const chatSetupCloseBtn = $('chatSetupCloseBtn');
  const chatSetupOkBtn = $('chatSetupOkBtn');
  const chatMessages = $('chatMessages');
  const chatBadge = $('chatBadge');
  const chatNote = $('chatNote');
  const chatInput = $('chatInput');
  const chatSendBtn = $('chatSendBtn');
  const chatInputRow = $('chatInputRow');

  let chatModels = [];
  let installedModelIds = new Set();
  let activeChatModel = null;
  let chatHistory = [];
  let currentPullCard = null;
  // null = noch nicht geprueft, sonst true/false. Steuert, welchen
  // Hinweistext das (bis zur Modellwahl deaktivierte) Eingabefeld zeigt.
  let ollamaKnownRunning = null;

  function escapeHtml(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function appendChatMessage(role, text) {
    const div = document.createElement('div');
    div.className = 'chat-message from-' + role;
    div.textContent = text;
    chatMessages.appendChild(div);
    chatMessages.scrollTop = chatMessages.scrollHeight;
  }

  function setChatSetupContent(html) {
    chatSetupContent.innerHTML = html;
  }

  function openChatSetupModal() {
    chatSetupOverlay.style.display = 'flex';
    runChatSetupCheck();
  }

  function closeChatSetupModal() {
    chatSetupOverlay.style.display = 'none';
  }

  // Eingabefeld bleibt deaktiviert, bis ein Modell aktiv ist, zeigt aber
  // je nach Stand als Platzhaltertext, was als naechstes zu tun ist,
  // statt nur "eingeben" ohne Erklaerung.
  function updateChatAvailability() {
    if (activeChatModel) {
      chatInput.disabled = false;
      chatSendBtn.disabled = false;
      chatInput.placeholder = 'Textbefehl eingeben…';
      return;
    }
    chatInput.disabled = true;
    chatSendBtn.disabled = true;
    if (ollamaKnownRunning === false) {
      chatInput.placeholder = 'Ollama läuft nicht – oben auf „Lokal“ klicken';
    } else if (ollamaKnownRunning === true) {
      chatInput.placeholder = 'Noch kein Modell gewählt – oben auf „Lokal“ klicken';
    } else {
      chatInput.placeholder = 'Prüfe Ollama…';
    }
  }

  function setChatBadgeState() {
    chatBadge.classList.remove('needs-setup', 'model-active');
    if (activeChatModel) {
      const model = chatModels.find((m) => m.id === activeChatModel);
      chatBadge.textContent = model ? model.label : activeChatModel;
      chatBadge.classList.add('model-active');
    } else {
      chatBadge.textContent = 'Lokal';
      if (ollamaKnownRunning !== null) chatBadge.classList.add('needs-setup');
    }
  }

  // Ollama haengt bei einem eigenen, frei waehlbaren Tag (z.B.
  // "gemma4:e2b") den Namen unveraendert, bei einem Modell ohne
  // ausdruecklichen Tag (z.B. "phi4-mini") aber automatisch ":latest" an,
  // siehe Ollama API Dokumentation, /api/tags. Diese Funktion gleicht
  // beide Faelle ab, statt nur auf exakte Gleichheit zu pruefen.
  function installedNameMatchesId(installedName, id) {
    if (installedName === id) return true;
    if (id.indexOf(':') === -1 && installedName === id + ':latest') return true;
    return false;
  }

  function activateChat(modelId) {
    activeChatModel = modelId;
    try { localStorage.setItem(ACTIVE_CHAT_MODEL_KEY, modelId); } catch (e) { /* egal */ }

    updateChatAvailability();
    setChatBadgeState();
    chatBadge.title = 'Anderes Modell wählen';

    const model = chatModels.find((m) => m.id === modelId);
    const label = model ? model.label : modelId;
    chatNote.textContent = 'Lokales Modell „' + label + '“, läuft komplett auf deinem Rechner über Ollama, keine Daten verlassen ihn.';

    if (chatMessages.children.length === 0) {
      appendChatMessage('system',
        'Bereit. Du kannst mich jetzt per Text steuern, zum Beispiel „schalte das ' +
        'Entrauschen aus“ oder „starte die Verarbeitung“. Videos fügst du weiterhin ' +
        'rechts per Drag and Drop hinzu.');
    }
    closeChatSetupModal();
    chatInput.focus();
  }

  function renderModelPicker() {
    const cards = chatModels.map((m) => {
      const installed = installedModelIds.has(m.id);
      return (
        '<div class="model-card" data-model-id="' + m.id + '">' +
          '<div class="model-card-head">' +
            '<span class="model-card-label">' + escapeHtml(m.label) + '</span>' +
            '<span class="model-card-ram">' + escapeHtml(m.ramHint) + '</span>' +
          '</div>' +
          '<div class="model-card-desc">' + escapeHtml(m.description) + '</div>' +
          '<div class="model-card-action">' +
            '<button class="secondary mc-action-btn" type="button">' +
              (installed ? 'Verwenden' : 'Herunterladen') +
            '</button>' +
            (installed ? '<span class="model-card-installed">installiert</span>' : '') +
          '</div>' +
          '<div class="model-card-progress" style="display:none;">' +
            '<div class="model-progress-bar"><div class="model-progress-fill" style="width:0%"></div></div>' +
            '<div class="model-progress-label"></div>' +
          '</div>' +
        '</div>'
      );
    }).join('');

    setChatSetupContent(
      '<div class="chat-setup-intro">Wähle ein lokales KI Modell für die Chatsteuerung. ' +
      'Läuft komplett auf diesem Rechner, es werden keine Daten verschickt.</div>' +
      '<div class="model-card-list">' + cards + '</div>'
    );

    chatSetupContent.querySelectorAll('.model-card').forEach((card) => {
      const modelId = card.dataset.modelId;
      card.querySelector('.mc-action-btn').addEventListener('click', () => onModelCardClick(modelId, card));
    });
  }

  async function onModelCardClick(modelId, card) {
    if (installedModelIds.has(modelId)) {
      activateChat(modelId);
      return;
    }
    const btn = card.querySelector('.mc-action-btn');
    const progressBox = card.querySelector('.model-card-progress');
    const fill = card.querySelector('.model-progress-fill');
    const label = card.querySelector('.model-progress-label');
    btn.disabled = true;
    btn.textContent = 'Lädt…';
    progressBox.style.display = 'block';
    label.textContent = 'Start…';

    currentPullCard = { modelId, fill, label };
    try {
      await window.__TAURI__.core.invoke('ollama_pull_model', { model: modelId });
      installedModelIds.add(modelId);
      activateChat(modelId);
    } catch (e) {
      console.error(e);
      label.textContent = 'Fehlgeschlagen: ' + String(e);
      btn.disabled = false;
      btn.textContent = 'Erneut versuchen';
    } finally {
      currentPullCard = null;
    }
  }

  async function initOllamaPullListener() {
    if (!isTauri()) return;
    await window.__TAURI__.event.listen('ollama-pull-progress', (event) => {
      if (!currentPullCard) return;
      const p = event.payload || {};
      if (p.model !== currentPullCard.modelId) return;
      if (p.total && p.completed) {
        const pct = Math.max(0, Math.min(100, Math.round((p.completed / p.total) * 100)));
        currentPullCard.fill.style.width = pct + '%';
        currentPullCard.label.textContent =
          pct + ' % (' + formatBytes(p.completed) + ' / ' + formatBytes(p.total) + ')';
      } else {
        currentPullCard.label.textContent = p.status || 'lädt…';
      }
    });
  }

  function showOllamaOfflineHint() {
    setChatSetupContent(
      '<div class="ollama-offline-box">' +
        '<strong>Ollama läuft gerade nicht.</strong> Für die Chatsteuerung einmalig ' +
        '<a href="https://ollama.com/download" target="_blank" rel="noopener">Ollama</a> ' +
        'installieren und starten, danach hier erneut prüfen.' +
      '</div>' +
      '<div class="actions"><button class="secondary" id="chatRecheckBtn" type="button">Erneut prüfen</button></div>'
    );
    const btn = $('chatRecheckBtn');
    if (btn) btn.addEventListener('click', runChatSetupCheck);
    updateChatAvailability();
    setChatBadgeState();
  }

  // Fragt Ollama Status, Modellliste und installierte Modelle ab und
  // aktualisiert dabei die globalen chatModels/installedModelIds/
  // ollamaKnownRunning. Gemeinsame Grundlage fuer den stillen Check beim
  // Start (initChat) und den sichtbaren Check im Popup
  // (runChatSetupCheck), damit beide nicht auseinanderlaufen.
  async function fetchChatSetupData() {
    if (!isTauri()) {
      ollamaKnownRunning = false;
      return { running: false, unavailable: true };
    }
    let status;
    try {
      status = await window.__TAURI__.core.invoke('ollama_status');
    } catch (e) {
      status = { running: false };
    }
    ollamaKnownRunning = !!(status && status.running);
    if (!ollamaKnownRunning) return { running: false };

    try {
      const [models, installed] = await Promise.all([
        window.__TAURI__.core.invoke('list_chat_models'),
        window.__TAURI__.core.invoke('ollama_installed_models'),
      ]);
      chatModels = models;
      const installedNames = installed.map((m) => m.name);
      installedModelIds = new Set(
        chatModels.filter((m) => installedNames.some((n) => installedNameMatchesId(n, m.id))).map((m) => m.id)
      );
      return { running: true };
    } catch (e) {
      console.error(e);
      return { running: true, error: e };
    }
  }

  // Stiller Check beim Anwendungsstart, oeffnet das Popup nicht. Ein
  // zuvor gewaehltes, weiterhin installiertes Modell wird automatisch
  // reaktiviert. Sonst bleibt das Popup einfach zu, Abzeichen und
  // Eingabefeld Platzhalter zeigen aber schon den erkannten Stand
  // (Ollama laeuft nicht / laeuft, aber noch kein Modell gewaehlt).
  async function initChat() {
    updateChatAvailability();
    const result = await fetchChatSetupData();
    if (result.running) {
      let saved = null;
      try { saved = localStorage.getItem(ACTIVE_CHAT_MODEL_KEY); } catch (e) { /* egal */ }
      if (saved && installedModelIds.has(saved)) {
        activateChat(saved);
        return;
      }
    }
    updateChatAvailability();
    setChatBadgeState();
  }

  // Fuellt das Popup selbst, aufgerufen beim Oeffnen (Klick auf das
  // Abzeichen) und beim "Erneut prüfen" Knopf.
  async function runChatSetupCheck() {
    setChatSetupContent('<div class="empty-hint">Prüfe, ob Ollama bereits läuft…</div>');
    if (!isTauri()) {
      setChatSetupContent('<div class="empty-hint">Chatsteuerung ist nur innerhalb der NovaStudioCast Anwendung verfügbar.</div>');
      return;
    }
    const result = await fetchChatSetupData();
    if (!result.running) {
      showOllamaOfflineHint();
      return;
    }
    if (result.error) {
      setChatSetupContent('<div class="status-error">Ollama antwortet, aber die Modellliste konnte nicht geladen werden: ' + escapeHtml(String(result.error)) + '</div>');
      updateChatAvailability();
      setChatBadgeState();
      return;
    }
    updateChatAvailability();
    setChatBadgeState();
    renderModelPicker();
  }

  chatBadge.addEventListener('click', openChatSetupModal);
  chatSetupCloseBtn.addEventListener('click', closeChatSetupModal);
  chatSetupOkBtn.addEventListener('click', closeChatSetupModal);
  chatSetupOverlay.addEventListener('click', (e) => {
    if (e.target === chatSetupOverlay) closeChatSetupModal();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && chatSetupOverlay.style.display !== 'none') closeChatSetupModal();
  });

  function buildChatContext() {
    return {
      clipNames: clips.map((c) => fileNameOf(c.path)),
      denoise: $('denoiseToggle').checked,
      loudnorm: loudnormToggle.checked,
      loudnormTarget: parseInt($('loudnormTarget').value, 10),
      marginSeconds: parseInt(marginSlider.value, 10) / 10,
      aiDisclosureActive: aiDisclosureToggle.checked,
    };
  }

  // Wendet einen einzelnen Werkzeugaufruf des Modells auf die echten
  // Bedienelemente an, genau als hätte der Nutzer selbst geklickt
  // (inklusive der bestehenden sync* Funktionen, damit z.B. die
  // Lautheit Auswahl korrekt ein oder ausgeblendet wird). Kein separates
  // Ausführen der Pipeline direkt aus dem Chat Modul heraus, siehe
  // src-tauri/src/chat/mod.rs Kopfkommentar für die Begründung.
  function applyChatToolCall(toolCall) {
    const args = toolCall.arguments || {};
    if (toolCall.name === 'set_pipeline_options') {
      const changed = [];
      if (typeof args.denoise === 'boolean') {
        $('denoiseToggle').checked = args.denoise;
        changed.push('Entrauschen ' + (args.denoise ? 'an' : 'aus'));
      }
      if (typeof args.loudnorm === 'boolean') {
        loudnormToggle.checked = args.loudnorm;
        syncLoudnormOptions();
        changed.push('Lautstärke angleichen ' + (args.loudnorm ? 'an' : 'aus'));
      }
      if (args.loudnormTarget !== undefined && args.loudnormTarget !== null) {
        const val = String(parseInt(args.loudnormTarget, 10));
        const select = $('loudnormTarget');
        if (Array.prototype.some.call(select.options, (o) => o.value === val)) {
          select.value = val;
          changed.push('Ziel Lautheit ' + val + ' LUFS');
        }
      }
      if (args.marginSeconds !== undefined && args.marginSeconds !== null) {
        const num = Number(args.marginSeconds);
        if (Number.isFinite(num)) {
          const clamped = Math.max(0, Math.min(1, num));
          marginSlider.value = String(Math.round(clamped * 10));
          updateMarginTag();
          changed.push('Zeitpuffer ' + clamped.toFixed(1).replace('.', ',') + ' s');
        }
      }
      if (typeof args.aiDisclosureActive === 'boolean') {
        aiDisclosureToggle.checked = args.aiDisclosureActive;
        syncAiDisclosureOptions();
        changed.push('KI Kennzeichnung ' + (args.aiDisclosureActive ? 'an' : 'aus'));
      }
      appendChatMessage('system', changed.length
        ? 'Einstellungen aktualisiert: ' + changed.join(', ') + '.'
        : 'Dazu gab es nichts zu ändern.');
      return;
    }
    if (toolCall.name === 'start_batch_pipeline') {
      if (clips.length === 0) {
        appendChatMessage('system', 'Es liegen noch keine Videos in der Liste, bitte erst rechts per Drag and Drop hinzufügen.');
        return;
      }
      appendChatMessage('system', 'Starte die Verarbeitung…');
      startPipeline();
      return;
    }
  }

  async function sendChatMessage() {
    const text = chatInput.value.trim();
    if (!text || !activeChatModel) return;
    chatInput.value = '';
    chatInput.disabled = true;
    chatSendBtn.disabled = true;
    appendChatMessage('user', text);
    chatHistory.push({ role: 'user', content: text });

    try {
      const reply = await window.__TAURI__.core.invoke('ollama_send_message', {
        model: activeChatModel,
        history: chatHistory,
        context: buildChatContext(),
      });
      const replyText = (reply.content || '').trim();
      const toolCalls = reply.toolCalls || [];
      if (replyText) {
        appendChatMessage('assistant', replyText);
        chatHistory.push({ role: 'assistant', content: replyText });
      } else if (toolCalls.length === 0) {
        appendChatMessage('assistant', 'Dazu ist mir gerade keine Antwort eingefallen, magst du es anders formulieren?');
      }
      toolCalls.forEach(applyChatToolCall);
    } catch (e) {
      console.error(e);
      appendChatMessage('system', 'Antwort fehlgeschlagen: ' + String(e));
    } finally {
      chatInput.disabled = false;
      chatSendBtn.disabled = false;
      chatInput.focus();
    }
  }

  chatSendBtn.addEventListener('click', sendChatMessage);
  chatInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendChatMessage();
    }
  });

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

  // Gemeinsamer Startweg für den "Start" Knopf und für einen
  // start_batch_pipeline Werkzeugaufruf aus dem Chat (siehe Abschnitt
  // "Chat / Ollama" weiter unten, applyChatToolCall), damit beide Wege
  // exakt dieselben jobs/options aus dem aktuellen Oberflächenzustand
  // bauen, statt die Logik doppelt zu pflegen.
  async function startPipeline() {
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
  }
  startBtn.addEventListener('click', startPipeline);

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
  initVersionTag();
  initProgressListener();
  initOllamaPullListener();
  checkTools();
  checkForUpdate(true);
  initChat();
})();
