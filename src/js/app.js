import { SkinViewer, IdleAnimation } from 'skinview3d';

// ==========================================
// NovaCraft Launcher — Main Application (v0.2.0)
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
  // Application State
  // ==========================================
  let selectedVersion = '';
  let selectedLoader = 'vanilla'; // 'vanilla' | 'fabric'
  let username = 'Steve';
  let ramMb = 2048;
  let versionFilter = 'release';
  let allVersions = [];
  let installedVersions = new Set();
  let recentUsernames = [];
  let closeOnLaunch = false;
  let isBusy = false;
  let customSkinBase64 = null;
  let activeInstanceId = 'default';

  // Persistent Instances Map
  let instances = [
    {
      id: 'default',
      name: 'Predefinito',
      version: '',
      loader: 'vanilla',
      ramMb: 2048,
      customDir: '',
    },
  ];

  // Modrinth State
  let currentModType = 'mod';
  let searchDebounceTimeout = null;
  let installedMods = [];

  // ==========================================
  // Settings & Storage
  // ==========================================
  const SETTINGS_KEY = 'novacraft-settings';
  const INSTANCES_KEY = 'novacraft-instances';

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

  // Initialize character
  if (customSkinBase64) {
    skinViewer.loadSkin(customSkinBase64);
  } else {
    updateSkin(username);
  }

  // ==========================================
  // Initialization
  // ==========================================

  try {
    allVersions = await invoke('get_versions');
    populateVersions();
    populateInstanceVersionSelect();
  } catch (e) {
    console.error('Failed to load versions:', e);
    versionSelect.innerHTML = '<option value="" disabled selected>Errore nel caricamento</option>';
    showError('Impossibile caricare le versioni', translateError(e));
  }

  await refreshInstalled();
  await refreshInstalledMods();
  renderInstancesList();

  // Load game directory & java detection
  try {
    gameDirPath.textContent = await invoke('get_game_dir');
  } catch {
    gameDirPath.textContent = '(non disponibile)';
  }

  try {
    const javaPath = await invoke('detect_java_cmd');
    if (javaPath) {
      statusJava.textContent = 'Java: ✓ trovato';
      statusJava.classList.add('found');
      javaInfo.textContent = 'Java di sistema presente (supporta anche Java 25+ gestito automaticamente).';
    } else {
      statusJava.textContent = 'Java: Auto-managed';
      javaInfo.textContent = 'Nessun Java di sistema: scaricato e gestito automaticamente da NovaCraft.';
    }
  } catch {
    statusJava.textContent = 'Java: Auto';
  }

  // Check if saved custom skin exists on disk
  try {
    const savedSkin = await invoke('get_custom_skin_cmd', { username });
    if (savedSkin && !customSkinBase64) {
      // Skin already saved
    }
  } catch {}

  // ==========================================
  // Event Listeners — Navigation
  // ==========================================

  Object.entries(navBtns).forEach(([viewName, btn]) => {
    btn.addEventListener('click', () => {
      showView(viewName);
      if (viewName === 'mods') {
        refreshInstalledMods();
        triggerModrinthSearch();
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
  // Event Listeners — Home View
  // ==========================================

  // Username input
  let skinTimeout;
  usernameInput.addEventListener('input', (e) => {
    username = e.target.value.trim() || 'Steve';
    saveAllSettings();
    if (!customSkinBase64) {
      clearTimeout(skinTimeout);
      skinTimeout = setTimeout(() => updateSkin(username), 500);
    } else {
      characterName.textContent = username;
    }
  });

  // Version selector
  versionSelect.addEventListener('change', (e) => {
    selectedVersion = e.target.value;
    updatePlayButton();
    checkFabricAvailability();
    saveAllSettings();
  });

  // RAM slider
  ramSlider.addEventListener('input', (e) => {
    ramMb = parseInt(e.target.value);
    ramValue.textContent = `${ramMb} MB`;
    saveAllSettings();
  });

  // Version filters (Release / Snapshot / Tutte)
  const filterButtons = versionFilterEl.querySelectorAll('.filter-btn');
  filterButtons.forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.filter === versionFilter);
    btn.addEventListener('click', () => {
      versionFilter = btn.dataset.filter;
      filterButtons.forEach((b) => b.classList.toggle('active', b === btn));
      saveAllSettings();
      populateVersions();
    });
  });

  // Loader Selector (Vanilla / Fabric)
  const loaderBtns = loaderSelector.querySelectorAll('.pill-btn');
  loaderBtns.forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.loader === selectedLoader);
    btn.addEventListener('click', () => {
      selectedLoader = btn.dataset.loader;
      loaderBtns.forEach((b) => b.classList.toggle('active', b === btn));
      statusLoader.textContent = selectedLoader === 'fabric' ? 'Fabric' : 'Vanilla';
      updateLoaderUI();
      saveAllSettings();
    });
  });

  function updateLoaderUI() {
    if (selectedLoader === 'fabric') {
      loaderStatusHint.style.display = 'flex';
      loaderStatusText.textContent = 'Fabric Mod Loader attivo';
    } else {
      loaderStatusHint.style.display = 'none';
    }
    updatePlayButton();
  }

  btnQuickModsFolder.addEventListener('click', () => {
    const activeInst = getActiveInstance();
    invoke('open_subfolder_cmd', {
      folderName: 'mods',
      customDir: activeInst.customDir || null,
    }).catch(console.error);
  });

  // Custom Skin Upload
  btnUploadSkin.addEventListener('click', () => skinFileInput.click());

  skinFileInput.addEventListener('change', async (e) => {
    const file = e.target.files && e.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async (ev) => {
      const dataUrl = ev.target.result;
      customSkinBase64 = dataUrl;
      skinViewer.loadSkin(dataUrl);
      characterName.textContent = `${username} (Custom)`;

      try {
        await invoke('save_custom_skin_cmd', {
          username,
          base64Png: dataUrl,
        });
      } catch (err) {
        console.error('Error saving skin:', err);
      }
      saveAllSettings();
    };
    reader.readAsDataURL(file);
  });

  btnResetSkin.addEventListener('click', () => {
    customSkinBase64 = null;
    updateSkin(username);
    saveAllSettings();
  });

  // Play button — Launch Minecraft
  playBtn.addEventListener('click', async () => {
    if (isBusy || !selectedVersion) return;

    isBusy = true;
    playBtn.disabled = true;
    hideError();
    addRecentUsername(username);

    const activeInst = getActiveInstance();
    let targetVersionId = selectedVersion;

    try {
      // 1. If Fabric is selected, ensure Fabric profile is generated
      if (selectedLoader === 'fabric') {
        playText.textContent = 'FABRIC…';
        showDownloadOverlay();
        progressStage.textContent = 'Verifica loader Fabric…';

        const availableLoaders = await invoke('fetch_fabric_loaders_cmd', {
          gameVersion: selectedVersion,
        });

        if (!availableLoaders || availableLoaders.length === 0) {
          throw new String(`Nessun Fabric loader disponibile per la versione ${selectedVersion}.`);
        }

        const latestLoader = availableLoaders[0];
        targetVersionId = await invoke('install_fabric_cmd', {
          gameVersion: selectedVersion,
          loaderVersion: latestLoader,
        });
      } else {
        // Vanilla download
        playText.textContent = 'SCARICO…';
        showDownloadOverlay();
        await invoke('download_version', { versionId: selectedVersion });
      }

      await refreshInstalled();

      // Launch the game
      hideDownloadOverlay();
      playText.textContent = 'AVVIO…';
      logToConsole(`[NovaCraft] Avvio Minecraft ${targetVersionId} (${username}, RAM: ${ramMb}MB)...`, 'info');

      await invoke('launch_game', {
        versionId: targetVersionId,
        username,
        ramMb,
        customDir: activeInst.customDir || null,
      });
    } catch (e) {
      console.error('Launch error:', e);
      hideDownloadOverlay();
      showError('Avvio non riuscito', translateError(e), typeof e === 'string' ? e : undefined);
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
  // Tauri Backend Event Listeners
  // ==========================================

  const STAGE_LABELS = {
    Client: 'Scarico il gioco',
    Libraries: 'Scarico le librerie',
    Natives: 'Componenti di sistema',
    Assets: 'Scarico le risorse (suoni, texture…)',
    Java: 'Scarico Java (necessario per questa versione)',
    Fabric: 'Installazione Fabric Loader',
    'Librerie Fabric': 'Librerie Fabric',
    Pronto: 'Completato!',
  };

  await listen('download-progress', (event) => {
    const { downloaded, total, percentage, stage } = event.payload;
    progressBarFill.style.width = `${Math.min(percentage, 100)}%`;
    progressText.textContent = `${Math.round(percentage)}%`;
    const label = STAGE_LABELS[stage] || stage;
    progressStage.textContent = total > 1 ? `${label} — ${downloaded}/${total}` : label;
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
    const { code, crashed, durationSecs, log } = payload;
    logToConsole(`[NovaCraft] Minecraft terminato (codice: ${code ?? '0'}, durata: ${durationSecs ?? 0}s).`, crashed ? 'stderr' : 'info');

    if (crashed) {
      const quick = (durationSecs ?? 0) < 15;
      const title = quick ? 'Il gioco non è partito' : 'Il gioco si è chiuso inaspettatamente';
      const message = quick
        ? 'Minecraft si è chiuso subito dopo l’avvio. Controlla la scheda Console Log per i dettagli.'
        : `Minecraft si è chiuso con un errore (codice ${code ?? '?'}).`;
      showError(title, message, tailLog(log));
    }
    resetPlayButton();
  });

  // Real-time Console Log streaming from Minecraft process
  await listen('game-log', (event) => {
    const { stream, text } = event.payload;
    logToConsole(text, stream === 'stderr' ? 'stderr' : 'stdout');
  });

  // ==========================================
  // Mods & Modrinth Logic
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
    const activeInst = getActiveInstance();
    invoke('open_subfolder_cmd', {
      folderName: currentModType === 'shader' ? 'shaderpacks' : currentModType === 'resourcepack' ? 'resourcepacks' : 'mods',
      customDir: activeInst.customDir || null,
    }).catch(console.error);
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
    const query = modSearchInput.value.trim();
    modsLoadingState.style.display = 'flex';
    modsEmptyState.style.display = 'none';
    modsResultsGrid.innerHTML = '';

    try {
      const loaderParam = selectedLoader === 'fabric' ? 'fabric' : null;
      const res = await invoke('search_modrinth_cmd', {
        query,
        projectType: currentModType,
        loader: currentModType === 'mod' ? loaderParam : null,
        gameVersion: selectedVersion || null,
        limit: 24,
        offset: 0,
      });

      modsLoadingState.style.display = 'none';

      if (!res.hits || res.hits.length === 0) {
        modsEmptyState.style.display = 'flex';
        return;
      }

      renderModSearchResults(res.hits);
    } catch (err) {
      modsLoadingState.style.display = 'none';
      console.error('Error fetching Modrinth projects:', err);
    }
  }

  function renderModSearchResults(hits) {
    modsResultsGrid.innerHTML = '';
    hits.forEach((item) => {
      const card = document.createElement('div');
      card.className = 'mod-card';

      const iconUrl = item.icon_url || 'https://cdn.modrinth.com/placeholder.svg';
      const categories = (item.categories || []).slice(0, 3);
      const isAlreadyInstalled = installedMods.some((m) =>
        m.filename.toLowerCase().includes(item.title.toLowerCase().replace(/\s+/g, ''))
      );

      card.innerHTML = `
        <img class="mod-icon" src="${escapeHtml(iconUrl)}" alt="" onerror="this.src='data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 24 24%22 fill=%22none%22 stroke=%22%2366727f%22><rect width=%2218%22 height=%2218%22 x=%223%22 y=%223%22 rx=%222%22/></svg>'">
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
            <button class="btn-install-mod ${isAlreadyInstalled ? 'installed' : ''}" type="button" data-id="${item.project_id}" data-type="${item.project_type || currentModType}">
              ${isAlreadyInstalled ? '✓ Installato' : 'Installa'}
            </button>
          </div>
        </div>
      `;

      const installBtn = card.querySelector('.btn-install-mod');
      if (!isAlreadyInstalled) {
        installBtn.addEventListener('click', async () => {
          await installModrinthProject(item, installBtn);
        });
      }

      modsResultsGrid.appendChild(card);
    });
  }

  async function installModrinthProject(project, buttonEl) {
    buttonEl.disabled = true;
    buttonEl.textContent = '…';

    try {
      const activeInst = getActiveInstance();
      const loaderParam = selectedLoader === 'fabric' ? 'fabric' : null;

      const versions = await invoke('get_modrinth_versions_cmd', {
        projectId: project.project_id,
        loader: currentModType === 'mod' ? loaderParam : null,
        gameVersion: selectedVersion || null,
      });

      if (!versions || versions.length === 0) {
        throw new Error('Nessun file compatibile trovato per questa versione di Minecraft.');
      }

      const targetVer = versions[0];
      const targetFile = targetVer.files.find((f) => f.primary) || targetVer.files[0];

      if (!targetFile) throw new Error('File non trovato nella release.');

      const folderType = currentModType === 'shader' ? 'shaderpacks' : currentModType === 'resourcepack' ? 'resourcepacks' : 'mods';

      await invoke('download_mod_file_cmd', {
        downloadUrl: targetFile.url,
        filename: targetFile.filename,
        folderType,
        customDir: activeInst.customDir || null,
      });

      // Se l'utente ha scaricato uno shader, assicuriamoci che Iris e Sodium siano installati per farlo funzionare!
      if (folderType === 'shaderpacks') {
        buttonEl.textContent = 'Configuro Iris…';
        const added = await ensureIrisAndSodiumInstalled(activeInst);
        if (added.length > 0) {
          logToConsole(`[NovaCraft] Shaderpack installato! Aggiunto automaticamente ${added.join(' e ')} (Fabric) per abilitare gli shader.`, 'info');
          showToast(
            'Shader pronto all\'uso! ✨',
            `Abbiamo aggiunto automaticamente ${added.join(' e ')} e attivato Fabric. Troverai lo shader in Opzioni > Grafica > Pacchetti Shader.`,
            true
          );
        }
      }

      buttonEl.textContent = '✓ Fatto';
      buttonEl.classList.add('installed');
      await refreshInstalledMods();
    } catch (e) {
      console.error('Install mod error:', e);
      buttonEl.disabled = false;
      buttonEl.textContent = 'Installa';
      showError('Errore download mod', translateError(e));
    }
  }

  async function ensureIrisAndSodiumInstalled(activeInst) {
    const hasIris = installedMods.some((m) => m.filename.toLowerCase().includes('iris'));
    const hasSodium = installedMods.some((m) => m.filename.toLowerCase().includes('sodium'));
    const autoInstalled = [];

    // Se Iris non c'è, scarichiamo Iris per Fabric
    if (!hasIris) {
      try {
        const irisVersions = await invoke('get_modrinth_versions_cmd', {
          projectId: 'iris',
          loader: 'fabric',
          gameVersion: selectedVersion || null,
        });
        if (irisVersions && irisVersions.length > 0) {
          const targetVer = irisVersions[0];
          const targetFile = targetVer.files.find((f) => f.primary) || targetVer.files[0];
          if (targetFile) {
            await invoke('download_mod_file_cmd', {
              downloadUrl: targetFile.url,
              filename: targetFile.filename,
              folderType: 'mods',
              customDir: activeInst.customDir || null,
            });
            autoInstalled.push('Iris Shaders');
          }
        }
      } catch (err) {
        console.warn('Impossibile scaricare Iris automaticamente:', err);
      }
    }

    // Se Sodium non c'è, scarichiamo Sodium per Fabric
    if (!hasSodium) {
      try {
        const sodiumVersions = await invoke('get_modrinth_versions_cmd', {
          projectId: 'sodium',
          loader: 'fabric',
          gameVersion: selectedVersion || null,
        });
        if (sodiumVersions && sodiumVersions.length > 0) {
          const targetVer = sodiumVersions[0];
          const targetFile = targetVer.files.find((f) => f.primary) || targetVer.files[0];
          if (targetFile) {
            await invoke('download_mod_file_cmd', {
              downloadUrl: targetFile.url,
              filename: targetFile.filename,
              folderType: 'mods',
              customDir: activeInst.customDir || null,
            });
            autoInstalled.push('Sodium');
          }
        }
      } catch (err) {
        console.warn('Impossibile scaricare Sodium automaticamente:', err);
      }
    }

    // Se abbiamo installato Iris o Sodium e il loader era Vanilla, passiamo a Fabric
    if (selectedLoader !== 'fabric') {
      selectedLoader = 'fabric';
      loaderBtns.forEach((b) => b.classList.toggle('active', b.dataset.loader === selectedLoader));
      statusLoader.textContent = 'Fabric';
      updateLoaderUI();
      saveAllSettings();
    }

    return autoInstalled;
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
          <span class="mod-file-name">${escapeHtml(m.filename.replace('.disabled', ''))}</span>
          <span class="mod-file-size">${sizeStr}</span>
        </div>
        <div class="mod-item-controls">
          <label class="settings-toggle" title="${m.is_enabled ? 'Disattiva mod' : 'Attiva mod'}">
            <input type="checkbox" class="chk-toggle-mod" ${m.is_enabled ? 'checked' : ''} />
            <span class="toggle-track"><span class="toggle-thumb"></span></span>
          </label>
          <button class="btn-icon-subtle btn-del-mod" title="Elimina file">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
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
          await refreshInstalledMods();
        } catch (e) {
          showError('Errore mod', String(e));
        }
      });

      const delBtn = item.querySelector('.btn-del-mod');
      delBtn.addEventListener('click', async () => {
        const activeInst = getActiveInstance();
        try {
          await invoke('delete_mod_cmd', {
            filename: m.filename,
            customDir: activeInst.customDir || null,
          });
          await refreshInstalledMods();
        } catch (e) {
          showError('Errore eliminazione mod', String(e));
        }
      });

      installedModsList.appendChild(item);
    });
  }

  // ==========================================
  // Instances & Profiles Logic
  // ==========================================

  btnShowNewInstance.addEventListener('click', () => {
    newInstanceCard.style.display = 'block';
    instNameInput.focus();
  });

  btnCancelNewInstance.addEventListener('click', () => {
    newInstanceCard.style.display = 'none';
  });

  btnDiscardInstance.addEventListener('click', () => {
    newInstanceCard.style.display = 'none';
  });

  instRamSlider.addEventListener('input', (e) => {
    instRamVal.textContent = `${e.target.value} MB`;
  });

  // Instance form loader pills
  let newInstLoader = 'vanilla';
  const instLoaderBtns = instLoaderSelector.querySelectorAll('.pill-btn');
  instLoaderBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      newInstLoader = btn.dataset.loader;
      instLoaderBtns.forEach((b) => b.classList.toggle('active', b === btn));
    });
  });

  btnCreateInstance.addEventListener('click', () => {
    const name = instNameInput.value.trim();
    if (!name) {
      instNameInput.focus();
      return;
    }

    const version = instVersionSelect.value || selectedVersion;
    const ram = parseInt(instRamSlider.value);
    const id = 'inst_' + Date.now();

    const newInst = {
      id,
      name,
      version,
      loader: newInstLoader,
      ramMb: ram,
      customDir: '', // Can be separated into instances/ folder if desired
    };

    instances.push(newInst);
    saveAllSettings();
    renderInstancesList();
    newInstanceCard.style.display = 'none';
    instNameInput.value = '';

    // Switch to the newly created instance
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
          <span>MC ${escapeHtml(inst.version || selectedVersion || 'Ultima')}</span>
          <span>•</span>
          <span style="text-transform: capitalize;">${escapeHtml(inst.loader)}</span>
          <span>•</span>
          <span>${inst.ramMb} MB RAM</span>
        </div>
        <div class="instance-actions">
          ${!isActive ? `<button class="btn-secondary btn-sm btn-activate-inst" type="button">Attiva</button>` : '<span></span>'}
          <div style="display: flex; gap: var(--space-xs);">
            ${inst.id !== 'default' ? `<button class="btn-icon-subtle btn-del-inst" title="Elimina profilo"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg></button>` : ''}
          </div>
        </div>
      `;

      const actBtn = card.querySelector('.btn-activate-inst');
      if (actBtn) {
        actBtn.addEventListener('click', () => activateInstance(inst.id));
      }

      const delBtn = card.querySelector('.btn-del-inst');
      if (delBtn) {
        delBtn.addEventListener('click', () => {
          instances = instances.filter((x) => x.id !== inst.id);
          if (activeInstanceId === inst.id) {
            activateInstance('default');
          }
          saveAllSettings();
          renderInstancesList();
        });
      }

      instancesListEl.appendChild(card);
    });
  }

  function activateInstance(id) {
    activeInstanceId = id;
    const inst = getActiveInstance();
    headerActiveProfile.textContent = inst.name;

    if (inst.version && allVersions.some((v) => v.id === inst.version)) {
      selectedVersion = inst.version;
      versionSelect.value = selectedVersion;
    }

    if (inst.loader) {
      selectedLoader = inst.loader;
      loaderBtns.forEach((b) => b.classList.toggle('active', b.dataset.loader === selectedLoader));
      statusLoader.textContent = selectedLoader === 'fabric' ? 'Fabric' : 'Vanilla';
      updateLoaderUI();
    }

    if (inst.ramMb) {
      ramMb = inst.ramMb;
      ramSlider.value = ramMb;
      ramValue.textContent = `${ramMb} MB`;
    }

    saveAllSettings();
    renderInstancesList();
    refreshInstalledMods();
    updatePlayButton();
  }

  function getActiveInstance() {
    return instances.find((i) => i.id === activeInstanceId) || instances[0];
  }

  function populateInstanceVersionSelect() {
    instVersionSelect.innerHTML = '';
    const releases = allVersions.filter((v) => v.version_type === 'release');
    releases.slice(0, 40).forEach((v) => {
      const opt = document.createElement('option');
      opt.value = v.id;
      opt.textContent = v.id;
      instVersionSelect.appendChild(opt);
    });
  }

  // ==========================================
  // Console Log Terminal Logic
  // ==========================================

  function logToConsole(text, type = 'stdout') {
    const line = document.createElement('div');
    line.className = `log-line log-${type}`;

    const now = new Date();
    const timeStr = `[${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}]`;

    line.textContent = `${timeStr} ${text}`;
    consoleOutput.appendChild(line);

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
      const text = consoleOutput.innerText;
      await navigator.clipboard.writeText(text);
      btnCopyLogs.textContent = '✓ Copiato!';
      setTimeout(() => (btnCopyLogs.textContent = 'Copia log'), 1500);
    } catch (e) {
      console.error(e);
    }
  });

  // ==========================================
  // Helper Functions
  // ==========================================

  function loadAllSettings() {
    try {
      const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY)) || {};
      if (saved.username) {
        username = saved.username;
        usernameInput.value = username;
      }
      if (saved.ramMb) {
        ramMb = saved.ramMb;
        ramSlider.value = ramMb;
        ramValue.textContent = `${ramMb} MB`;
      }
      if (saved.loader) {
        selectedLoader = saved.loader;
        statusLoader.textContent = selectedLoader === 'fabric' ? 'Fabric' : 'Vanilla';
      }
      if (saved.filter) versionFilter = saved.filter;
      if (Array.isArray(saved.usernames)) recentUsernames = saved.usernames;
      if (saved.customSkin) customSkinBase64 = saved.customSkin;
      closeOnLaunch = saved.closeOnLaunch === true;
      optCloseOnLaunch.checked = closeOnLaunch;

      const savedInst = JSON.parse(localStorage.getItem(INSTANCES_KEY));
      if (Array.isArray(savedInst) && savedInst.length > 0) {
        instances = savedInst;
      }
      if (saved.activeInstanceId) {
        activeInstanceId = saved.activeInstanceId;
      }

      const activeInst = getActiveInstance();
      headerActiveProfile.textContent = activeInst.name;

      refreshUsernameList();
      updateLoaderUI();
    } catch (e) {
      console.error('Error loading settings:', e);
    }
  }

  function saveAllSettings() {
    try {
      localStorage.setItem(
        SETTINGS_KEY,
        JSON.stringify({
          username,
          version: selectedVersion,
          loader: selectedLoader,
          ramMb,
          filter: versionFilter,
          closeOnLaunch,
          usernames: recentUsernames,
          customSkin: customSkinBase64,
          activeInstanceId,
        })
      );
      localStorage.setItem(INSTANCES_KEY, JSON.stringify(instances));
    } catch (e) {
      console.error('Error saving settings:', e);
    }
  }

  function populateVersions() {
    const filtered =
      versionFilter === 'all'
        ? allVersions
        : allVersions.filter((v) => v.version_type === versionFilter);

    versionSelect.innerHTML = '';

    if (filtered.length === 0) {
      versionSelect.innerHTML = '<option value="" disabled selected>Nessuna versione</option>';
      selectedVersion = '';
      return;
    }

    filtered.forEach((v) => {
      const option = document.createElement('option');
      option.value = v.id;
      option.textContent = installedVersions.has(v.id) ? `${v.id}  ✓` : v.id;
      versionSelect.appendChild(option);
    });

    let target;
    if (selectedVersion && filtered.some((v) => v.id === selectedVersion)) {
      target = selectedVersion;
    } else {
      target = filtered[0].id;
    }
    selectedVersion = target;
    versionSelect.value = target;
    updatePlayButton();
    checkFabricAvailability();
  }

  async function checkFabricAvailability() {
    if (selectedLoader !== 'fabric') return;
    try {
      const loaders = await invoke('fetch_fabric_loaders_cmd', { gameVersion: selectedVersion });
      if (!loaders || loaders.length === 0) {
        loaderStatusText.textContent = `Nessun Fabric per ${selectedVersion}`;
      } else {
        loaderStatusText.textContent = `Fabric ${loaders[0]} pronto`;
      }
    } catch {
      loaderStatusText.textContent = 'Fabric (auto)';
    }
  }

  function showError(title, message, details) {
    showToast(title || 'Errore', message || '', false, details);
  }

  function showToast(title, message, isSuccess = false, details = null) {
    errorTitle.textContent = title;
    errorTitle.style.color = isSuccess ? 'var(--accent)' : 'var(--text-error)';
    errorMessage.textContent = message;
    if (details) {
      errorDetails.textContent = details;
      errorDetails.style.display = 'block';
    } else {
      errorDetails.style.display = 'none';
      errorDetails.textContent = '';
    }
    errorToast.classList.add('show');
    if (isSuccess) {
      setTimeout(() => errorToast.classList.remove('show'), 6000);
    }
  }

  function hideError() {
    errorToast.classList.remove('show');
  }

  function translateError(e) {
    const msg = typeof e === 'string' ? e : (e && e.message) || String(e);
    const l = msg.toLowerCase();
    if (l.includes('sending request') || l.includes('dns') || l.includes('connect') ||
        l.includes('timed out') || l.includes('network') || l.includes('reqwest')) {
      return 'Sembra un problema di connessione a internet. Controlla la rete e riprova.';
    }
    if (l.includes('java')) {
      return 'Java non trovato o non compatibile. NovaCraft lo scaricherà automaticamente.';
    }
    if (l.includes('sha1') || l.includes('hash') || l.includes('corrupt')) {
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
    const isInstalled = installedVersions.has(selectedVersion);
    if (selectedLoader === 'fabric') {
      playText.textContent = isInstalled ? 'GIOCA CON FABRIC' : 'INSTALLA FABRIC & GIOCA';
    } else {
      playText.textContent = isInstalled ? 'GIOCA' : 'SCARICA E GIOCA';
    }
  }

  async function refreshInstalled() {
    try {
      installedVersions = new Set(await invoke('list_installed_versions'));
    } catch (e) {
      console.error('Failed to list installed versions:', e);
    }
    populateVersions();
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
    return lines.slice(-12).join('\n');
  }

  function updateSkin(name) {
    const encoded = encodeURIComponent(name);
    characterName.textContent = name;
    skinViewer.loadSkin(`https://mc-heads.net/skin/${encoded}`).catch(() => {
      skinViewer.loadSkin('https://mc-heads.net/skin/Steve');
    });
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
    if (!str) return '';
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
});
