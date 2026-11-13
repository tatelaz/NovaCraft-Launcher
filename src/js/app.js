import { SkinViewer, IdleAnimation } from 'skinview3d';

// ==========================================
// NovaCraft Launcher — Main Application
// ==========================================

document.addEventListener('DOMContentLoaded', async () => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  // ---- DOM Elements (Home) ----
  const versionSelect = document.getElementById('version-select');
  const usernameInput = document.getElementById('username-input');
  const skinCanvas = document.getElementById('skin-canvas');
  const characterName = document.getElementById('character-name');
  const playBtn = document.getElementById('play-btn');
  const playText = document.getElementById('play-text');
  const ramSlider = document.getElementById('ram-slider');
  const ramValue = document.getElementById('ram-value');
  const downloadOverlay = document.getElementById('download-overlay');
  const progressBarFill = document.getElementById('progress-bar-fill');
  const progressText = document.getElementById('progress-text');
  const progressStage = document.getElementById('progress-stage');
  const statusJava = document.getElementById('status-java');
  const statusLoader = document.getElementById('status-loader');
  const statusDir = document.getElementById('status-dir');
  const versionFilterEl = document.getElementById('version-filter');
  const errorToast = document.getElementById('error-toast');
  const errorTitle = document.getElementById('error-title');
  const errorMessage = document.getElementById('error-message');
  const errorDetails = document.getElementById('error-details');
  const errorClose = document.getElementById('error-close');
  const usernameList = document.getElementById('username-list');
  const headerActiveProfile = document.getElementById('active-profile-name');

  // ---- Loader Selector (Vanilla / Fabric) ----
  const loaderSelector = document.getElementById('loader-selector');
  const loaderBtns = loaderSelector.querySelectorAll('.pill-btn');
  const loaderStatusHint = document.getElementById('loader-status-hint');
  const loaderStatusText = document.getElementById('loader-status-text');
  const btnQuickModsFolder = document.getElementById('btn-quick-mods-folder');

  // ---- Skin Custom Upload ----
  const skinFileInput = document.getElementById('skin-file-input');
  const btnUploadSkin = document.getElementById('btn-upload-skin');
  const btnResetSkin = document.getElementById('btn-reset-skin');

  // ---- Views & Navigation ----
  const navBtns = {
    home: document.getElementById('nav-home'),
    mods: document.getElementById('nav-mods'),
    instances: document.getElementById('nav-instances'),
    logs: document.getElementById('nav-logs'),
    settings: document.getElementById('nav-settings'),
  };

  const views = {
    home: document.getElementById('view-home'),
    mods: document.getElementById('view-mods'),
    instances: document.getElementById('view-instances'),
    logs: document.getElementById('view-logs'),
    settings: document.getElementById('view-settings'),
  };

  // ---- Mods View Elements ----
  const tabBrowseMods = document.getElementById('tab-browse-mods');
  const tabInstalledMods = document.getElementById('tab-installed-mods');
  const sectionModrinth = document.getElementById('section-modrinth');
  const sectionInstalled = document.getElementById('section-installed');
  const modSearchInput = document.getElementById('mod-search-input');
  const modSearchClear = document.getElementById('mod-search-clear');
  const modTypeFilter = document.getElementById('mod-type-filter');
  const modsResultsGrid = document.getElementById('mods-results-grid');
  const modsEmptyState = document.getElementById('mods-empty-state');
  const modsEmptyText = modsEmptyState.querySelector('p');
  const modsLoadingState = document.getElementById('mods-loading-state');
  const installedModsList = document.getElementById('installed-mods-list');
  const installedEmptyState = document.getElementById('installed-empty-state');
  const installedModsCount = document.getElementById('installed-mods-count');
  const btnOpenModsFolder = document.getElementById('btn-open-mods-folder');
  const btnGoBrowseMods = document.getElementById('btn-go-browse-mods');

  // ---- Instances View Elements ----
  const btnShowNewInstance = document.getElementById('btn-show-new-instance');
  const newInstanceCard = document.getElementById('new-instance-card');
  const btnCancelNewInstance = document.getElementById('btn-cancel-new-instance');
  const btnDiscardInstance = document.getElementById('btn-discard-instance');
  const btnCreateInstance = document.getElementById('btn-create-instance');
  const instNameInput = document.getElementById('inst-name-input');
  const instVersionSelect = document.getElementById('inst-version-select');
  const instLoaderSelector = document.getElementById('inst-loader-selector');
  const instRamSlider = document.getElementById('inst-ram-slider');
  const instRamVal = document.getElementById('inst-ram-val');
  const instancesListEl = document.getElementById('instances-list');

  // ---- Logs View Elements ----
  const consoleOutput = document.getElementById('console-output');
  const chkAutoscroll = document.getElementById('chk-autoscroll');
  const btnCopyLogs = document.getElementById('btn-copy-logs');
  const btnClearLogs = document.getElementById('btn-clear-logs');

  // ---- Settings Elements ----
  const gameDirPath = document.getElementById('game-dir-path');
  const javaInfo = document.getElementById('java-info');
  const btnOpenFolder = document.getElementById('btn-open-folder');
  const btnOpenRuntimes = document.getElementById('btn-open-runtimes');
  const optCloseOnLaunch = document.getElementById('opt-close-on-launch');

  // ==========================================
  // Constants & Application State
  // ==========================================
  const SETTINGS_KEY = 'novacraft-settings';
  const INSTANCES_KEY = 'novacraft-instances';
  const SETTINGS_SCHEMA = 2;
  /** The console keeps at most this many lines (older ones are dropped). */
  const MAX_LOG_LINES = 3000;
  /** Minecraft player names only allow letters, digits and underscore. */
  const INVALID_NAME_CHARS = /[^A-Za-z0-9_]/g;
  const MODS_EMPTY_DEFAULT = modsEmptyText.textContent;

  // The values below always mirror the ACTIVE profile (instance).
  let selectedVersion = '';
  let selectedLoader = 'vanilla'; // 'vanilla' | 'fabric'
  let ramMb = 2048;

  let username = 'Steve';
  let versionFilter = 'release';
  let allVersions = [];
  let installedVersions = new Set();
  let recentUsernames = [];
  let closeOnLaunch = false;
  let isBusy = false;
  let activeInstanceId = 'default';
  let instances = [defaultInstance()];
  let legacySkinToMigrate = null;

  // Modrinth State
  let currentModType = 'mod';
  let searchDebounceTimeout = null;
  let searchSeq = 0;
  let fabricCheckSeq = 0;
  let skinSeq = 0;
  let installedMods = [];
  let installedProjectIds = new Set();

  function defaultInstance() {
    return { id: 'default', name: 'Predefinito', version: '', loader: 'vanilla', ramMb: 2048, customDir: '' };
  }

  // ==========================================
  // Startup
  // ==========================================

  loadAllSettings();

  // 3D character viewer
  const skinViewer = new SkinViewer({
    canvas: skinCanvas,
    width: 260,
    height: 380,
  });
  skinViewer.animation = new IdleAnimation();
  skinViewer.fov = 40;
  skinViewer.zoom = 0.9;
  skinViewer.controls.enableZoom = false;
  skinViewer.controls.enablePan = false;

  await migrateLegacySkin();
  loadSkinForUser();
  showAppVersion();
  await migrateInstanceDirs();

  try {
    allVersions = await invoke('get_versions');
  } catch (e) {
    console.error('Failed to load versions:', e);
    showError('Impossibile caricare le versioni', translateError(e), rawMessage(e));
  }
  ensureFilterShows(selectedVersion);
  await refreshInstalled(); // also fills the version menu
  if (allVersions.length === 0) {
    versionSelect.innerHTML = '<option value="" disabled selected>Nessuna versione disponibile</option>';
  }
  populateInstanceVersionSelect();
  renderInstancesList();
  refreshInstalledMods();

  invoke('get_game_dir')
    .then((dir) => (gameDirPath.textContent = dir))
    .catch(() => (gameDirPath.textContent = '(non disponibile)'));
  refreshJavaStatus();

  // ==========================================
  // Navigation
  // ==========================================

  Object.entries(navBtns).forEach(([viewName, btn]) => {
    btn.addEventListener('click', () => {
      showView(viewName);
      if (viewName === 'mods') {
        refreshInstalledMods();
        triggerModrinthSearch();
      }
      // Lines that arrived while the console was hidden: jump to the newest.
      if (viewName === 'logs' && chkAutoscroll.checked) {
        consoleOutput.scrollTop = consoleOutput.scrollHeight;
      }
    });
  });

  function showView(name) {
    Object.entries(views).forEach(([vName, el]) => {
      el.classList.toggle('active', vName === name);
    });
    Object.entries(navBtns).forEach(([vName, btn]) => {
      btn.classList.toggle('active', vName === name);
    });
  }

  // ==========================================
  // Home View
  // ==========================================

  // Username input (only characters Minecraft accepts)
  let skinTimeout;
  usernameInput.addEventListener('input', (e) => {
    const clean = e.target.value.replace(INVALID_NAME_CHARS, '');
    if (clean !== e.target.value) e.target.value = clean;
    username = clean || 'Steve';
    characterName.textContent = username;
    saveAllSettings();
    clearTimeout(skinTimeout);
    skinTimeout = setTimeout(loadSkinForUser, 500);
  });

  // Version selector
  versionSelect.addEventListener('change', (e) => {
    selectedVersion = e.target.value;
    updateActiveInstance({ version: selectedVersion });
    updatePlayButton();
    checkFabricAvailability();
  });

  // RAM slider
  ramSlider.addEventListener('input', (e) => {
    ramMb = parseInt(e.target.value, 10);
    ramValue.textContent = `${ramMb} MB`;
    updateActiveInstance({ ramMb });
  });

  // Version filters (Release / Snapshot / Tutte)
  const filterButtons = versionFilterEl.querySelectorAll('.filter-btn');
  updateFilterButtons();
  filterButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      versionFilter = btn.dataset.filter;
      updateFilterButtons();
      saveAllSettings();
      populateVersions();
    });
  });

  function updateFilterButtons() {
    versionFilterEl
      .querySelectorAll('.filter-btn')
      .forEach((b) => b.classList.toggle('active', b.dataset.filter === versionFilter));
  }

  // Loader Selector (Vanilla / Fabric)
  loaderBtns.forEach((btn) => {
    btn.addEventListener('click', () => setLoader(btn.dataset.loader));
  });
  updateLoaderUI();

  function setLoader(loader) {
    selectedLoader = loader === 'fabric' ? 'fabric' : 'vanilla';
    updateActiveInstance({ loader: selectedLoader });
    updateLoaderUI();
    checkFabricAvailability();
  }

  function updateLoaderUI() {
    loaderBtns.forEach((b) => b.classList.toggle('active', b.dataset.loader === selectedLoader));
    statusLoader.textContent = selectedLoader === 'fabric' ? 'Fabric' : 'Vanilla';
    if (selectedLoader === 'fabric') {
      loaderStatusHint.style.display = 'flex';
      loaderStatusText.textContent = 'Fabric Mod Loader attivo';
    } else {
      loaderStatusHint.style.display = 'none';
    }
    updatePlayButton();
  }

  btnQuickModsFolder.addEventListener('click', () => {
    openProfileFolder('mods');
  });

  // Custom Skin Upload
  btnUploadSkin.addEventListener('click', () => skinFileInput.click());

  skinFileInput.addEventListener('change', async (e) => {
    const file = e.target.files && e.target.files[0];
    skinFileInput.value = ''; // so the same file can be chosen again
    if (!file) return;
    try {
      const dataUrl = await normalizeSkin(file);
      await invoke('save_custom_skin_cmd', { username, base64Png: dataUrl });
      await loadSkinForUser();
      showToast(
        'Skin caricata ✓',
        `La skin di ${username} si vedrà anche in gioco (in giocatore singolo) dal prossimo avvio.`,
        true
      );
    } catch (err) {
      console.error('Error saving skin:', err);
      showError('Skin non valida', translateError(err));
    }
  });

  btnResetSkin.addEventListener('click', async () => {
    try {
      await invoke('delete_custom_skin_cmd', { username });
    } catch (err) {
      showError('Impossibile rimuovere la skin', translateError(err));
    }
    await loadSkinForUser();
  });

  // Play button — download what's missing, then launch Minecraft
  playBtn.addEventListener('click', async () => {
    if (isBusy || !selectedVersion) return;

    isBusy = true;
    playBtn.disabled = true;
    hideError();
    addRecentUsername(username);

    const activeInst = getActiveInstance();
    let targetVersionId = selectedVersion;

    try {
      // 1. The vanilla game + the right Java (the progress bar stays visible).
      playText.textContent = 'SCARICO…';
      showDownloadOverlay();
      await invoke('download_version', { versionId: selectedVersion });

      // 2. Fabric on top, if chosen.
      if (selectedLoader === 'fabric') {
        playText.textContent = 'FABRIC…';
        progressStage.textContent = 'Installazione di Fabric…';
        targetVersionId = await invoke('ensure_fabric_cmd', { gameVersion: selectedVersion });
      }

      await refreshInstalled();

      // 3. Launch
      hideDownloadOverlay();
      playText.textContent = 'AVVIO…';
      logToConsole(
        `[NovaCraft] Avvio Minecraft ${targetVersionId} (${username}, RAM: ${ramMb} MB, profilo: ${activeInst.name})...`,
        'info'
      );

      await invoke('launch_game', {
        versionId: targetVersionId,
        username,
        ramMb,
        customDir: activeInst.customDir || null,
      });
    } catch (e) {
      console.error('Launch error:', e);
      hideDownloadOverlay();
      showError('Avvio non riuscito', translateError(e), rawMessage(e));
      resetPlayButton();
    }
  });

  // Settings: open folders
  btnOpenFolder.addEventListener('click', () => {
    invoke('open_game_folder', { which: null }).catch((e) =>
      showError('Impossibile aprire la cartella', translateError(e))
    );
  });
  btnOpenRuntimes.addEventListener('click', () => {
    invoke('open_game_folder', { which: 'runtimes' }).catch((e) =>
      showError('Impossibile aprire la cartella', translateError(e))
    );
  });
  optCloseOnLaunch.addEventListener('change', (e) => {
    closeOnLaunch = e.target.checked;
    saveAllSettings();
  });
  errorClose.addEventListener('click', hideError);

  // ==========================================
  // Tauri Backend Events
  // ==========================================

  const STAGE_LABELS = {
    Client: 'Scarico il gioco',
    Libraries: 'Scarico le librerie',
    Natives: 'Componenti di sistema',
    Assets: 'Scarico le risorse (suoni, texture…)',
    Java: 'Scarico Java (necessario per questa versione)',
    Fabric: 'Installazione di Fabric',
    'Librerie Fabric': 'Scarico le librerie di Fabric',
    Pronto: 'Completato!',
  };

  await listen('download-progress', (event) => {
    const { downloaded, total, percentage, stage } = event.payload;
    progressBarFill.style.width = `${Math.min(percentage, 100)}%`;
    progressText.textContent = `${Math.round(percentage)}%`;
    const label = STAGE_LABELS[stage] || stage;
    if (stage === 'Java' && total > 1) {
      progressStage.textContent = `${label} — ${formatMb(downloaded)} / ${formatMb(total)}`;
    } else {
      progressStage.textContent = total > 1 ? `${label} — ${downloaded}/${total}` : label;
    }
  });

  await listen('game-started', (event) => {
    logToConsole(`[NovaCraft] Gioco avviato con successo! (PID: ${event.payload})`, 'info');
    if (closeOnLaunch) {
      invoke('close_launcher').catch(() => {});
      return;
    }
    playText.textContent = '✓ IN GIOCO';
    playBtn.disabled = true;
    isBusy = true;
  });

  await listen('game-exited', (event) => {
    const payload = event.payload || {};
    const { code, crashed, durationSecs, log, crashReport } = payload;
    logToConsole(
      `[NovaCraft] Minecraft terminato (codice: ${code ?? '?'}, durata: ${durationSecs ?? 0}s).`,
      crashed ? 'stderr' : 'info'
    );

    if (crashed) {
      const quick = (durationSecs ?? 0) < 15;
      const title = quick ? 'Il gioco non è partito' : 'Il gioco si è chiuso inaspettatamente';
      const message = quick
        ? 'Minecraft si è chiuso subito dopo l’avvio. Qui sotto le ultime righe del log (tutto il resto è nella scheda Console).'
        : `Minecraft si è chiuso con un errore (codice ${code ?? '?'}). Qui sotto le ultime righe del log.`;
      const details = [crashReport ? `Report del crash: ${crashReport}` : '', tailLog(log) || '']
        .filter(Boolean)
        .join('\n\n');
      showError(title, message, details || undefined);
    }
    resetPlayButton();
  });

  // Real-time console log streaming from the Minecraft process
  await listen('game-log', (event) => {
    const { stream, text } = event.payload;
    logToConsole(text, stream === 'stderr' ? 'stderr' : 'stdout');
  });

  // ==========================================
  // Mods & Modrinth
  // ==========================================

  tabBrowseMods.addEventListener('click', () => {
    tabBrowseMods.classList.add('active');
    tabInstalledMods.classList.remove('active');
    sectionModrinth.style.display = 'block';
    sectionInstalled.style.display = 'none';
  });

  tabInstalledMods.addEventListener('click', () => {
    tabInstalledMods.classList.add('active');
    tabBrowseMods.classList.remove('active');
    sectionModrinth.style.display = 'none';
    sectionInstalled.style.display = 'block';
    refreshInstalledMods();
  });

  btnGoBrowseMods.addEventListener('click', () => {
    tabBrowseMods.click();
  });

  btnOpenModsFolder.addEventListener('click', () => {
    openProfileFolder(folderForType(currentModType));
  });

  // Mod category filter pills
  const modFilterPills = modTypeFilter.querySelectorAll('.filter-pill');
  modFilterPills.forEach((pill) => {
    pill.addEventListener('click', () => {
      currentModType = pill.dataset.type;
      modFilterPills.forEach((p) => p.classList.toggle('active', p === pill));
      triggerModrinthSearch();
    });
  });

  modSearchInput.addEventListener('input', () => {
    const q = modSearchInput.value.trim();
    modSearchClear.style.display = q ? 'block' : 'none';
    clearTimeout(searchDebounceTimeout);
    searchDebounceTimeout = setTimeout(triggerModrinthSearch, 400);
  });

  modSearchClear.addEventListener('click', () => {
    modSearchInput.value = '';
    modSearchClear.style.display = 'none';
    triggerModrinthSearch();
  });

  async function triggerModrinthSearch() {
    // Only the most recent search may update the page (answers can arrive out of order).
    const seq = ++searchSeq;
    const query = modSearchInput.value.trim();
    const type = currentModType;
    const activeInst = getActiveInstance();
    modsLoadingState.style.display = 'flex';
    modsEmptyState.style.display = 'none';
    modsResultsGrid.innerHTML = '';

    try {
      const [res, ids] = await Promise.all([
        invoke('search_modrinth_cmd', {
          query,
          projectType: type,
          gameVersion: selectedVersion || null,
          limit: 24,
          offset: 0,
        }),
        invoke('installed_project_ids_cmd', {
          projectType: type,
          customDir: activeInst.customDir || null,
        }).catch(() => []),
      ]);
      if (seq !== searchSeq) return;

      installedProjectIds = new Set(ids);
      modsLoadingState.style.display = 'none';

      if (!res.hits || res.hits.length === 0) {
        modsEmptyText.textContent = MODS_EMPTY_DEFAULT;
        modsEmptyState.style.display = 'flex';
        return;
      }

      renderModSearchResults(res.hits);
    } catch (err) {
      if (seq !== searchSeq) return;
      console.error('Error fetching Modrinth projects:', err);
      modsLoadingState.style.display = 'none';
      modsEmptyText.textContent = `Impossibile contattare Modrinth. ${translateError(err)}`;
      modsEmptyState.style.display = 'flex';
    }
  }

  function renderModSearchResults(hits) {
    modsResultsGrid.innerHTML = '';
    hits.forEach((item) => {
      const card = document.createElement('div');
      card.className = 'mod-card';

      const categories = (item.categories || []).slice(0, 3);
      const isAlreadyInstalled = installedProjectIds.has(item.project_id);

      card.innerHTML = `
        <img class="mod-icon" src="${escapeHtml(item.icon_url || PLACEHOLDER_ICON)}" alt="">
        <div class="mod-info">
          <div class="mod-header">
            <span class="mod-title" title="${escapeHtml(item.title)}">${escapeHtml(item.title)}</span>
          </div>
          <span class="mod-author">di ${escapeHtml(item.author || 'Sconosciuto')} • ⬇ ${formatDownloads(item.downloads)}</span>
          <p class="mod-desc">${escapeHtml(item.description || 'Nessuna descrizione.')}</p>
          <div class="mod-meta-row">
            <div class="mod-tags">
              ${categories.map((c) => `<span class="mod-tag">${escapeHtml(c)}</span>`).join('')}
            </div>
            <button class="btn-install-mod ${isAlreadyInstalled ? 'installed' : ''}" type="button">
              ${isAlreadyInstalled ? '✓ Installato' : 'Installa'}
            </button>
          </div>
        </div>
      `;

      const icon = card.querySelector('.mod-icon');
      icon.addEventListener('error', () => {
        if (icon.src !== PLACEHOLDER_ICON) icon.src = PLACEHOLDER_ICON;
      });

      const installBtn = card.querySelector('.btn-install-mod');
      if (!isAlreadyInstalled) {
        installBtn.addEventListener('click', () => installModrinthProject(item, installBtn));
      }

      modsResultsGrid.appendChild(card);
    });
  }

  async function installModrinthProject(project, buttonEl) {
    if (!selectedVersion) {
      showError('Nessuna versione scelta', 'Scegli prima una versione di Minecraft nella Home.');
      return;
    }

    const type = currentModType;
    buttonEl.disabled = true;
    buttonEl.textContent = 'Installo…';

    try {
      const activeInst = getActiveInstance();
      const report = await invoke('install_modrinth_project_cmd', {
        projectId: project.project_id,
        projectType: type,
        gameVersion: selectedVersion,
        customDir: activeInst.customDir || null,
      });

      buttonEl.textContent = '✓ Installato';
      buttonEl.classList.add('installed');
      installedProjectIds.add(project.project_id);

      // Mods and shaders only work with Fabric (shaders through Iris).
      let switchedToFabric = false;
      if ((type === 'mod' || type === 'shader') && selectedLoader !== 'fabric') {
        setLoader('fabric');
        switchedToFabric = true;
      }

      const extras = report.already_installed ? report.installed : report.installed.slice(1);
      const parts = [];
      if (report.already_installed && report.installed.length === 0) parts.push('Era già installato.');
      if (extras.length) parts.push(`Aggiunto anche quello che serve per farlo funzionare: ${extras.join(', ')}.`);
      if (switchedToFabric) parts.push('Ho attivato Fabric per questo profilo: serve per usare mod e shader.');
      if (type === 'shader') parts.push('In gioco lo trovi in Opzioni › Grafica › Pacchetti shader.');
      if (report.warnings.length) parts.push(`Attenzione: ${report.warnings.join(' ')}`);
      if (parts.length) {
        showToast(`${project.title} installato`, parts.join(' '), report.warnings.length === 0);
      }

      await refreshInstalledMods();
    } catch (e) {
      console.error('Install mod error:', e);
      buttonEl.disabled = false;
      buttonEl.textContent = 'Installa';
      showError(`Impossibile installare ${project.title}`, translateError(e), rawMessage(e));
    }
  }

  async function refreshInstalledMods() {
    try {
      const activeInst = getActiveInstance();
      installedMods = await invoke('list_installed_mods_cmd', {
        customDir: activeInst.customDir || null,
      });
      installedModsCount.textContent = installedMods.length;
      renderInstalledModsList();
    } catch (e) {
      console.error('Error listing installed mods:', e);
    }
  }

  function renderInstalledModsList() {
    installedModsList.innerHTML = '';
    if (!installedMods || installedMods.length === 0) {
      installedEmptyState.style.display = 'flex';
      return;
    }

    installedEmptyState.style.display = 'none';

    installedMods.forEach((m) => {
      const item = document.createElement('div');
      item.className = `installed-mod-item ${m.is_enabled ? '' : 'disabled'}`;

      const sizeKb = Math.round(m.size_bytes / 1024);
      const sizeStr = sizeKb > 1024 ? `${(sizeKb / 1024).toFixed(1)} MB` : `${sizeKb} KB`;

      item.innerHTML = `
        <div style="display: flex; align-items: baseline;">
          <span class="mod-file-name">${escapeHtml(m.filename.replace(/\.disabled$/, ''))}</span>
          <span class="mod-file-size">${sizeStr}</span>
        </div>
        <div class="mod-item-controls">
          <label class="settings-toggle" title="${m.is_enabled ? 'Disattiva mod' : 'Attiva mod'}">
            <input type="checkbox" class="chk-toggle-mod" ${m.is_enabled ? 'checked' : ''} />
            <span class="toggle-track"><span class="toggle-thumb"></span></span>
          </label>
          <button class="btn-icon-subtle btn-del-mod" type="button" title="Elimina file">
            ${TRASH_ICON}
          </button>
        </div>
      `;

      const chk = item.querySelector('.chk-toggle-mod');
      chk.addEventListener('change', async () => {
        const activeInst = getActiveInstance();
        try {
          await invoke('toggle_mod_cmd', {
            filename: m.filename,
            customDir: activeInst.customDir || null,
          });
        } catch (e) {
          showError('Errore mod', translateError(e));
        }
        await refreshInstalledMods();
      });

      const delBtn = item.querySelector('.btn-del-mod');
      delBtn.addEventListener('click', async () => {
        const activeInst = getActiveInstance();
        try {
          await invoke('delete_mod_cmd', {
            filename: m.filename,
            customDir: activeInst.customDir || null,
          });
        } catch (e) {
          showError('Errore eliminazione mod', translateError(e));
        }
        await refreshInstalledMods();
      });

      installedModsList.appendChild(item);
    });
  }

  // ==========================================
  // Instances & Profiles
  // ==========================================

  btnShowNewInstance.addEventListener('click', () => {
    populateInstanceVersionSelect();
    newInstanceCard.style.display = 'block';
    instNameInput.focus();
  });

  btnCancelNewInstance.addEventListener('click', closeNewInstanceForm);
  btnDiscardInstance.addEventListener('click', closeNewInstanceForm);

  function closeNewInstanceForm() {
    newInstanceCard.style.display = 'none';
    instNameInput.value = '';
    setNewInstLoader('vanilla');
  }

  instRamSlider.addEventListener('input', (e) => {
    instRamVal.textContent = `${e.target.value} MB`;
  });

  // Instance form loader pills
  let newInstLoader = 'vanilla';
  const instLoaderBtns = instLoaderSelector.querySelectorAll('.pill-btn');
  instLoaderBtns.forEach((btn) => {
    btn.addEventListener('click', () => setNewInstLoader(btn.dataset.loader));
  });

  function setNewInstLoader(loader) {
    newInstLoader = loader;
    instLoaderBtns.forEach((b) => b.classList.toggle('active', b.dataset.loader === loader));
  }

  btnCreateInstance.addEventListener('click', async () => {
    const name = instNameInput.value.trim();
    if (!name) {
      instNameInput.focus();
      return;
    }

    const id = 'inst_' + Date.now();
    let customDir;
    try {
      // Every new profile gets its own folder: own mods, worlds and settings.
      customDir = await invoke('instance_dir_cmd', { instanceId: id });
    } catch (e) {
      showError('Impossibile creare il profilo', translateError(e));
      return;
    }

    instances.push({
      id,
      name,
      version: instVersionSelect.value || selectedVersion,
      loader: newInstLoader,
      ramMb: parseInt(instRamSlider.value, 10),
      customDir,
    });
    closeNewInstanceForm();
    activateInstance(id);
  });

  function renderInstancesList() {
    instancesListEl.innerHTML = '';
    instances.forEach((inst) => {
      const isActive = inst.id === activeInstanceId;
      const card = document.createElement('div');
      card.className = `instance-card ${isActive ? 'active-instance' : ''}`;

      card.innerHTML = `
        <div class="instance-top">
          <span class="instance-name">${escapeHtml(inst.name)}</span>
          ${isActive ? '<span class="instance-badge">Attivo</span>' : ''}
        </div>
        <div class="instance-details">
          <span>MC ${escapeHtml(inst.version || 'ultima release')}</span>
          <span>•</span>
          <span>${inst.loader === 'fabric' ? 'Fabric' : 'Vanilla'}</span>
          <span>•</span>
          <span>${inst.ramMb} MB RAM</span>
        </div>
        <div class="instance-actions">
          ${!isActive ? '<button class="btn-secondary btn-sm btn-activate-inst" type="button">Attiva</button>' : '<span></span>'}
          <div style="display: flex; gap: var(--space-xs); align-items: center;">
            <button class="btn-secondary btn-sm btn-open-inst" type="button" title="Apri la cartella del profilo">Cartella</button>
            ${inst.id !== 'default' ? `<button class="btn-icon-subtle btn-del-inst" type="button" title="Elimina profilo">${TRASH_ICON}</button>` : ''}
          </div>
        </div>
      `;

      const actBtn = card.querySelector('.btn-activate-inst');
      if (actBtn) {
        actBtn.addEventListener('click', () => activateInstance(inst.id));
      }

      card.querySelector('.btn-open-inst').addEventListener('click', () => {
        invoke('open_subfolder_cmd', { folderName: '', customDir: inst.customDir || null }).catch((e) =>
          showError('Impossibile aprire la cartella', translateError(e))
        );
      });

      // Deleting needs a second click, to avoid accidents.
      const delBtn = card.querySelector('.btn-del-inst');
      if (delBtn) {
        let armed = false;
        let disarmTimer;
        delBtn.addEventListener('click', () => {
          if (!armed) {
            armed = true;
            delBtn.classList.add('confirm');
            delBtn.textContent = 'Elimina?';
            delBtn.title = 'Clicca di nuovo per eliminare il profilo';
            disarmTimer = setTimeout(() => renderInstancesList(), 4000);
            return;
          }
          clearTimeout(disarmTimer);
          instances = instances.filter((x) => x.id !== inst.id);
          if (activeInstanceId === inst.id) {
            activateInstance('default');
          } else {
            saveAllSettings();
            renderInstancesList();
          }
          showToast(
            'Profilo eliminato',
            `I mondi e le mod di "${inst.name}" non sono stati cancellati: restano in ${inst.customDir || 'cartella principale'}.`,
            true
          );
        });
      }

      instancesListEl.appendChild(card);
    });
  }

  function activateInstance(id) {
    activeInstanceId = instances.some((i) => i.id === id) ? id : 'default';
    applyInstanceValues(getActiveInstance());
    ensureFilterShows(selectedVersion);
    populateVersions();
    saveAllSettings();
    renderInstancesList();
    refreshInstalledMods();
    if (views.mods.classList.contains('active')) triggerModrinthSearch();
  }

  /** Copy a profile's values into the Home controls. */
  function applyInstanceValues(inst) {
    selectedVersion = inst.version || '';
    selectedLoader = inst.loader === 'fabric' ? 'fabric' : 'vanilla';
    ramMb = inst.ramMb || 2048;
    ramSlider.value = ramMb;
    ramValue.textContent = `${ramMb} MB`;
    headerActiveProfile.textContent = inst.name;
    statusDir.textContent = `Profilo: ${inst.name}`;
    updateLoaderUI();
  }

  /** Save a change made on the Home page into the active profile. */
  function updateActiveInstance(fields) {
    Object.assign(getActiveInstance(), fields);
    saveAllSettings();
    renderInstancesList();
  }

  function getActiveInstance() {
    return instances.find((i) => i.id === activeInstanceId) || instances[0];
  }

  function populateInstanceVersionSelect() {
    instVersionSelect.innerHTML = '';
    const releases = allVersions.filter((v) => v.version_type === 'release').slice(0, 40);
    releases.forEach((v) => {
      const opt = document.createElement('option');
      opt.value = v.id;
      opt.textContent = v.id;
      instVersionSelect.appendChild(opt);
    });
    if (releases.some((v) => v.id === selectedVersion)) instVersionSelect.value = selectedVersion;
  }

  /** Profiles created before folders existed: give each its own folder now. */
  async function migrateInstanceDirs() {
    let changed = false;
    for (const inst of instances) {
      if (inst.id !== 'default' && !inst.customDir) {
        try {
          inst.customDir = await invoke('instance_dir_cmd', { instanceId: inst.id });
          changed = true;
        } catch (e) {
          console.error('Instance dir error:', e);
        }
      }
    }
    if (changed) saveAllSettings();
  }

  async function openProfileFolder(folderName) {
    const activeInst = getActiveInstance();
    try {
      await invoke('open_subfolder_cmd', { folderName, customDir: activeInst.customDir || null });
    } catch (e) {
      showError('Impossibile aprire la cartella', translateError(e));
    }
  }

  // ==========================================
  // Console Log
  // ==========================================

  function logToConsole(text, type = 'stdout') {
    const line = document.createElement('div');
    line.className = `log-line log-${type}`;

    const now = new Date();
    const timeStr = `[${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}]`;

    line.textContent = `${timeStr} ${text}`;
    consoleOutput.appendChild(line);

    // Long sessions would otherwise slow the launcher down.
    while (consoleOutput.childElementCount > MAX_LOG_LINES) {
      consoleOutput.firstElementChild.remove();
    }

    if (chkAutoscroll.checked) {
      consoleOutput.scrollTop = consoleOutput.scrollHeight;
    }
  }

  btnClearLogs.addEventListener('click', () => {
    consoleOutput.innerHTML = '';
    logToConsole('Console pulita.', 'info');
  });

  btnCopyLogs.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(consoleOutput.innerText);
      btnCopyLogs.textContent = '✓ Copiato!';
      setTimeout(() => (btnCopyLogs.textContent = 'Copia log'), 1500);
    } catch (e) {
      console.error(e);
    }
  });

  // ==========================================
  // Skin
  // ==========================================

  /** Show the custom skin of the current player, or their online skin. */
  async function loadSkinForUser() {
    const seq = ++skinSeq;
    const name = username;
    let dataUrl = null;
    try {
      dataUrl = await invoke('get_custom_skin_cmd', { username: name });
    } catch (e) {
      console.error('Skin error:', e);
    }
    if (seq !== skinSeq) return;

    characterName.textContent = name;
    btnResetSkin.style.display = dataUrl ? '' : 'none';
    btnResetSkin.title = 'Torna alla skin del tuo account';

    const source = dataUrl || `https://mc-heads.net/skin/${encodeURIComponent(name)}`;
    Promise.resolve(skinViewer.loadSkin(source)).catch(() =>
      Promise.resolve(skinViewer.loadSkin('https://mc-heads.net/skin/Steve')).catch(() => {})
    );
  }

  /** The first version stored ONE skin for everybody in the settings: move it to the current player. */
  async function migrateLegacySkin() {
    if (!legacySkinToMigrate) return;
    try {
      const existing = await invoke('get_custom_skin_cmd', { username });
      if (!existing) await invoke('save_custom_skin_cmd', { username, base64Png: legacySkinToMigrate });
    } catch (e) {
      console.error('Skin migration error:', e);
    }
    legacySkinToMigrate = null;
    saveAllSettings();
  }

  /**
   * Check a skin file and return it as a PNG data URL.
   * Old 64×32 skins are converted to the modern 64×64 layout (like Minecraft does),
   * so they look right in game too.
   */
  function normalizeSkin(file) {
    return new Promise((resolve, reject) => {
      if (file.type && file.type !== 'image/png') {
        reject(new Error('La skin deve essere un file .png'));
        return;
      }
      const url = URL.createObjectURL(file);
      const img = new Image();
      img.onload = () => {
        URL.revokeObjectURL(url);
        const w = img.naturalWidth;
        const h = img.naturalHeight;
        const square = w === h;
        const legacy = w === h * 2;
        if (w < 64 || w % 64 !== 0 || !(square || legacy)) {
          reject(new Error(`Dimensioni non valide (${w}×${h}). Una skin deve essere 64×64 (o il vecchio formato 64×32).`));
          return;
        }
        const canvas = document.createElement('canvas');
        canvas.width = w;
        canvas.height = w;
        const ctx = canvas.getContext('2d', { willReadFrequently: true });
        ctx.drawImage(img, 0, 0);
        if (legacy) convertLegacySkin(ctx, w / 64);
        resolve(canvas.toDataURL('image/png'));
      };
      img.onerror = () => {
        URL.revokeObjectURL(url);
        reject(new Error('Impossibile leggere l’immagine.'));
      };
      img.src = url;
    });
  }

  /** Same mirrored copies Minecraft makes to turn a 64×32 skin into 64×64 (left arm and leg). */
  function convertLegacySkin(ctx, scale) {
    const copies = [
      [4, 16, 16, 32, 4, 4], [8, 16, 16, 32, 4, 4], [0, 20, 24, 32, 4, 12], [4, 20, 16, 32, 4, 12],
      [8, 20, 8, 32, 4, 12], [12, 20, 16, 32, 4, 12], [44, 16, -8, 32, 4, 4], [48, 16, -8, 32, 4, 4],
      [40, 20, 0, 32, 4, 12], [44, 20, -8, 32, 4, 12], [48, 20, -16, 32, 4, 12], [52, 20, -8, 32, 4, 12],
    ];
    for (const [x, y, dx, dy, w, h] of copies) {
      const sw = w * scale;
      const sh = h * scale;
      const src = ctx.getImageData(x * scale, y * scale, sw, sh);
      const out = ctx.createImageData(sw, sh);
      for (let row = 0; row < sh; row++) {
        for (let col = 0; col < sw; col++) {
          const si = (row * sw + col) * 4;
          const di = (row * sw + (sw - 1 - col)) * 4;
          out.data.set(src.data.subarray(si, si + 4), di);
        }
      }
      ctx.putImageData(out, (x + dx) * scale, (y + dy) * scale);
    }
  }

  // ==========================================
  // Settings & Storage
  // ==========================================

  function loadAllSettings() {
    let saved = {};
    try {
      saved = JSON.parse(localStorage.getItem(SETTINGS_KEY)) || {};
    } catch (e) {
      console.error('Error loading settings:', e);
    }
    try {
      const savedInst = JSON.parse(localStorage.getItem(INSTANCES_KEY));
      if (Array.isArray(savedInst) && savedInst.length > 0) instances = savedInst;
    } catch (e) {
      console.error('Error loading profiles:', e);
    }
    if (!instances.some((i) => i.id === 'default')) instances.unshift(defaultInstance());

    if (saved.username) username = String(saved.username).replace(INVALID_NAME_CHARS, '') || 'Steve';
    if (['release', 'snapshot', 'all'].includes(saved.filter)) versionFilter = saved.filter;
    if (Array.isArray(saved.usernames)) recentUsernames = saved.usernames;
    closeOnLaunch = saved.closeOnLaunch === true;
    if (saved.activeInstanceId && instances.some((i) => i.id === saved.activeInstanceId)) {
      activeInstanceId = saved.activeInstanceId;
    }

    // Older settings kept version / loader / RAM outside the profiles (and the
    // version was never read back): move them into the active profile once.
    if (saved.schema !== SETTINGS_SCHEMA) {
      const inst = getActiveInstance();
      if (saved.version) inst.version = saved.version;
      if (saved.loader) inst.loader = saved.loader;
      if (saved.ramMb) inst.ramMb = saved.ramMb;
      if (saved.customSkin) legacySkinToMigrate = saved.customSkin;
    }

    usernameInput.value = username;
    characterName.textContent = username;
    optCloseOnLaunch.checked = closeOnLaunch;
    applyInstanceValues(getActiveInstance());
    refreshUsernameList();
    saveAllSettings();
  }

  function saveAllSettings() {
    try {
      localStorage.setItem(
        SETTINGS_KEY,
        JSON.stringify({
          schema: SETTINGS_SCHEMA,
          username,
          filter: versionFilter,
          closeOnLaunch,
          usernames: recentUsernames,
          activeInstanceId,
          // Kept until the old skin has been moved to its own file
          ...(legacySkinToMigrate ? { customSkin: legacySkinToMigrate } : {}),
        })
      );
      localStorage.setItem(INSTANCES_KEY, JSON.stringify(instances));
    } catch (e) {
      console.error('Error saving settings:', e);
    }
  }

  // ==========================================
  // Versions
  // ==========================================

  /** Make sure the Release/Snapshot/All filter shows the given version. */
  function ensureFilterShows(versionId) {
    const v = allVersions.find((x) => x.id === versionId);
    if (!v || versionFilter === 'all' || v.version_type === versionFilter) return;
    versionFilter = ['release', 'snapshot'].includes(v.version_type) ? v.version_type : 'all';
    updateFilterButtons();
    saveAllSettings();
  }

  function populateVersions() {
    const filtered =
      versionFilter === 'all' ? allVersions : allVersions.filter((v) => v.version_type === versionFilter);

    if (filtered.length === 0) {
      versionSelect.innerHTML = '<option value="" disabled selected>Nessuna versione</option>';
      return;
    }

    versionSelect.innerHTML = '';
    filtered.forEach((v) => {
      const option = document.createElement('option');
      option.value = v.id;
      option.textContent = installedVersions.has(v.id) ? `${v.id}  ✓` : v.id;
      versionSelect.appendChild(option);
    });

    if (!filtered.some((v) => v.id === selectedVersion)) {
      selectedVersion = filtered[0].id;
      updateActiveInstance({ version: selectedVersion });
    }
    versionSelect.value = selectedVersion;
    updatePlayButton();
    checkFabricAvailability();
  }

  async function checkFabricAvailability() {
    if (selectedLoader !== 'fabric' || !selectedVersion) return;
    const seq = ++fabricCheckSeq;
    const version = selectedVersion;
    loaderStatusText.textContent = 'Controllo Fabric…';
    try {
      const loaders = await invoke('fetch_fabric_loaders_cmd', { gameVersion: version });
      if (seq !== fabricCheckSeq) return;
      loaderStatusText.textContent =
        !loaders || loaders.length === 0 ? `Fabric non disponibile per ${version}` : `Fabric ${loaders[0]} pronto`;
    } catch {
      if (seq !== fabricCheckSeq) return;
      loaderStatusText.textContent = hasFabricInstalled(version) ? 'Fabric installato (offline)' : 'Fabric (serve internet)';
    }
  }

  function hasFabricInstalled(version) {
    return [...installedVersions].some((id) => id.startsWith('fabric-loader-') && id.endsWith(`-${version}`));
  }

  async function refreshInstalled() {
    try {
      installedVersions = new Set(await invoke('list_installed_versions'));
    } catch (e) {
      console.error('Failed to list installed versions:', e);
    }
    populateVersions();
  }

  async function refreshJavaStatus() {
    try {
      const js = await invoke('java_status_cmd');
      const managed = js.managed || [];
      if (js.system_major) {
        statusJava.textContent = `Java ${js.system_major} ✓`;
        statusJava.classList.add('found');
      } else if (managed.length) {
        statusJava.textContent = `Java ${managed.join(', ')} ✓`;
        statusJava.classList.add('found');
      } else {
        statusJava.textContent = 'Java: automatico';
      }
      const parts = [
        js.system_path
          ? `Java ${js.system_major} installato sul PC (${js.system_path}).`
          : 'Nessun Java installato sul PC.',
      ];
      if (managed.length) parts.push(`Già scaricati da NovaCraft: Java ${managed.join(', ')}.`);
      javaInfo.textContent = parts.join(' ');
    } catch {
      statusJava.textContent = 'Java: automatico';
      javaInfo.textContent = 'NovaCraft scarica in automatico il Java giusto quando serve.';
    }
  }

  async function showAppVersion() {
    try {
      const v = await window.__TAURI__.app.getVersion();
      document.querySelectorAll('.app-version').forEach((el) => (el.textContent = `v${v}`));
    } catch {
      /* keep the version written in the page */
    }
  }

  // ==========================================
  // Helpers
  // ==========================================

  function showError(title, message, details) {
    showToast(title || 'Errore', message || '', false, details);
  }

  function showToast(title, message, isSuccess = false, details = null) {
    errorTitle.textContent = title;
    errorTitle.style.color = isSuccess ? 'var(--accent)' : 'var(--text-error)';
    errorMessage.textContent = message;
    if (details && details !== message) {
      errorDetails.textContent = details;
      errorDetails.style.display = 'block';
    } else {
      errorDetails.style.display = 'none';
      errorDetails.textContent = '';
    }
    errorToast.classList.add('show');
    clearTimeout(showToast.timer);
    if (isSuccess) {
      showToast.timer = setTimeout(() => errorToast.classList.remove('show'), 8000);
    }
  }

  function hideError() {
    errorToast.classList.remove('show');
  }

  function rawMessage(e) {
    return typeof e === 'string' ? e : (e && e.message) || String(e);
  }

  /** Turn a technical error into something understandable. */
  function translateError(e) {
    const msg = rawMessage(e);
    const l = msg.toLowerCase();
    if (
      l.includes('error sending request') || l.includes('dns') || l.includes('timed out') ||
      l.includes('connection') || l.includes('errore di rete') || l.includes('impossibile connettersi')
    ) {
      return 'Sembra un problema di connessione a internet. Controlla la rete e riprova.';
    }
    if (l.includes('sha-1 mismatch') || l.includes('danneggiato')) {
      return 'Un file scaricato risulta danneggiato. Riprova: verrà riscaricato.';
    }
    return msg;
  }

  function resetPlayButton() {
    playBtn.disabled = false;
    isBusy = false;
    updatePlayButton();
  }

  function updatePlayButton() {
    if (isBusy) return;
    const vanillaInstalled = installedVersions.has(selectedVersion);
    if (selectedLoader === 'fabric') {
      playText.textContent =
        vanillaInstalled && hasFabricInstalled(selectedVersion) ? 'GIOCA CON FABRIC' : 'INSTALLA FABRIC E GIOCA';
    } else {
      playText.textContent = vanillaInstalled ? 'GIOCA' : 'SCARICA E GIOCA';
    }
  }

  function refreshUsernameList() {
    usernameList.innerHTML = '';
    recentUsernames.forEach((n) => {
      const opt = document.createElement('option');
      opt.value = n;
      usernameList.appendChild(opt);
    });
  }

  function addRecentUsername(name) {
    const n = (name || '').trim();
    if (!n) return;
    recentUsernames = [n, ...recentUsernames.filter((x) => x !== n)].slice(0, 6);
    saveAllSettings();
    refreshUsernameList();
  }

  function tailLog(log) {
    if (!log) return undefined;
    const lines = log.split('\n').filter((l) => l.trim());
    if (lines.length === 0) return undefined;
    return lines.slice(-15).join('\n');
  }

  function folderForType(type) {
    if (type === 'shader') return 'shaderpacks';
    if (type === 'resourcepack') return 'resourcepacks';
    return 'mods';
  }

  function showDownloadOverlay() {
    downloadOverlay.classList.add('active');
    progressBarFill.style.width = '0%';
    progressText.textContent = '0%';
    progressStage.textContent = 'Preparazione…';
  }

  function hideDownloadOverlay() {
    downloadOverlay.classList.remove('active');
  }

  function escapeHtml(str) {
    if (str === null || str === undefined) return '';
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }

  function formatDownloads(num) {
    if (!num) return '0';
    if (num >= 1000000) return (num / 1000000).toFixed(1) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(1) + 'K';
    return String(num);
  }

  function formatMb(bytes) {
    return `${Math.round(bytes / 1048576)} MB`;
  }
});

const PLACEHOLDER_ICON =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%2366727f'%3E%3Crect width='18' height='18' x='3' y='3' rx='2'/%3E%3C/svg%3E";

const TRASH_ICON =
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>';
