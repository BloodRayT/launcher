import { appendConsole, bindConsole } from './console.js';
import { byId, fitPreview, hideError, invoke, openUrl, showError, tauriCore, tauriDialog, tauriEvent } from './core.js';
import { BUILD_IDS, BUILDS, buildById, buildCaption, buildChips } from './builds.js';
import { startEmbers } from './embers.js';
import { isLiveWallpaperOn, startWallpaper } from './wallpaper.js';
import { bindVersionPicker } from './versions.js';
import { DEFAULT_JVM_ARGS, readShowModified, renderCustomArgs, writeShowModified } from './launch-options.js';
import { LOCAL_NEWS_IMAGES, homeCards, newsArticlesHtml, releasesHtml, twitchHtml } from './news.js';
import { SERVER_DEFAULTS, autoJoinState, serversHtml } from './servers.js';
import { SHOP_URL, shopGridHtml } from './shop.js';
import { DEFAULT_SKIN, createSkinStage, readSkinFile } from './skin.js';
import { openSkinCatalog } from './skin-catalog.js';
import { bindHeroVideo } from './hero-video.js';
import { CLOSE_AFTER_LAUNCH_MS, applyMotion, isCloseAfterLaunch, isMotionOff, setCloseAfterLaunch, setMotionOff } from './motion.js';
import { bindElyAccount, isElySignedIn, renderElyBox } from './ely-account.js';
import { checkForUpdates } from './updates.js';
import { bindAvatar } from './avatar.js';
import { closeGlassModal, openGlassModal, openModalKind, replaceModalBody } from './modal.js';
import { readList, readNumber, writeList, writeNumber } from './storage.js';
import { changedSlots, connectivityLabel, escapeHtml, isValidUsername, unseenPostIds } from './utils.js';

const NEWS_CACHE_KEY = 'fenix.news.v2';
const NEWS_SEEN_KEY = 'fenix.news.seen';
const RELEASES_CACHE_KEY = 'fenix.releases.v1';
const RELEASES_FETCHED_KEY = 'fenix.releases.fetchedAt';
const RELEASES_REFRESH_MS = 30 * 60 * 1000;
const NEWS_REFRESH_MS = 60 * 1000;
const NEWS_FOCUS_REFRESH_MS = 20 * 1000;
const REFRESH_INTERVAL_MS = 30000;
/** Онлайн серверов обновляется часто: игрок видит, как заходят и выходят игроки. */
const SERVERS_REFRESH_MS = 10000;
const CONNECTIVITY_INTERVAL_MS = 60000;
const SAVE_DEBOUNCE_MS = 300;
const MAX_SKIN_BYTES = 32 * 1024;
const TABS = ['home', 'shop', 'settings', 'faq', 'console'];

let currentConfig = {};

let pendingSave = Promise.resolve();

/** Сохранения идут строго по очереди, иначе одновременные изменения затирали бы друг друга. */
function persistConfig(changes) {
  pendingSave = pendingSave.then(async () => {
    try {
      currentConfig = await invoke('save_launcher_config', { newConfig: { ...currentConfig, ...changes } });
    } catch (error) {
      // Интерфейс возвращается к тому, что реально лежит на диске.
      currentConfig = await invoke('get_config').catch(() => currentConfig);
      showError({ title: 'Настройки не сохранены', message: String(error), hint: 'Проверьте свободное место на диске.' });
    }
    return currentConfig;
  });
  return pendingSave;
}

// --- Вкладки ---

function switchTab(tabId) {
  for (const id of TABS) byId(`tab-${id}`)?.classList.add('hidden');
  (byId(`tab-${tabId}`) || byId('tab-home')).classList.remove('hidden');
  document.querySelectorAll('.nav-btn[data-tab]').forEach((button) => {
    button.classList.toggle('active-nav', button.dataset.tab === tabId);
  });
}

// --- Сборки и баннер ---

function selectedBuild() {
  return buildById(currentConfig.selectedVersion);
}

let latestRelease = '';
/** Ролик на баннере сборки FenixCraft (hero-video.js). */
let heroVideo = null;

function renderBuilds() {
  const selected = selectedBuild();
  // Своя версия из списка не совпадает ни с одной карточкой: фокус остаётся на первой.
  const focusId = selected.custom ? BUILD_IDS[0] : selected.id;
  byId('build-strip').innerHTML = BUILDS.map((build) => {
    const checked = build.id === selected.id;
    const caption = build.id === 'latest-vanilla' && latestRelease ? `${latestRelease} · последняя` : buildCaption(build);
    return `
      <button role="radio" aria-checked="${checked}" tabindex="${build.id === focusId ? 0 : -1}" data-build="${build.id}" class="build-card">
        <span class="build-name">${escapeHtml(build.name)}</span>
        <span class="build-meta">${escapeHtml(caption)}</span>
      </button>`;
  }).join('');
  byId('version-picker-value').textContent = selected.custom ? selected.name : 'Выбрать из списка';
  document.querySelector('.version-picker').classList.toggle('is-active', Boolean(selected.custom));
}

function renderHero(animate) {
  const build = selectedBuild();
  const hero = document.querySelector('.hero');
  hero.dataset.loader = build.loader;
  hero.dataset.shaders = String(build.shaders);
  // Адрес картинки абсолютный: относительный разрешился бы от файла стилей, а не от страницы.
  hero.style.setProperty('--hero-image', `url("${new URL(build.banner, document.baseURI).href}")`);
  // Названия FenixCraft и FenixClient в цветах чата Minecraft: Fenix — §e, остальное — §b.
  // Vanilla целиком жёлтая (§e).
  const brand = build.name.match(/^Fenix(\w+)$/);
  let title = escapeHtml(build.name);
  if (brand) title = `<span class="mc-yellow">Fenix</span><span class="mc-aqua">${escapeHtml(brand[1])}</span>`;
  else if (build.name === 'Vanilla') title = '<span class="mc-yellow">Vanilla</span>';
  byId('hero-title').innerHTML = title;
  heroVideo?.update(build);
  byId('hero-meta').innerHTML = buildChips(build, latestRelease).map((chip) => `<span class="hero-chip">${escapeHtml(chip)}</span>`).join('');
  byId('hero-desc').textContent = build.desc;
  if (animate) {
    hero.classList.remove('hero-swap');
    void hero.offsetWidth;
    hero.classList.add('hero-swap');
  }
}

async function selectBuild(id, { focus = false } = {}) {
  if (buildById(id).id !== id || id === currentConfig.selectedVersion) return;
  currentConfig = { ...currentConfig, selectedVersion: id };
  renderBuilds();
  renderHero(true);
  renderAutoJoin();
  if (focus) document.querySelector(`[data-build="${id}"]`)?.focus();
  await persistConfig({ selectedVersion: id });
  if (currentConfig.selectedVersion !== id) {
    renderBuilds();
    renderHero(false);
  }
}

/** Стрелки переключают сборку, как в обычной группе переключателей. */
function bindBuildKeys() {
  byId('build-strip').addEventListener('keydown', (event) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (!step) return;
    event.preventDefault();
    const index = Math.max(0, BUILD_IDS.indexOf(selectedBuild().id));
    selectBuild(BUILD_IDS[(index + step + BUILD_IDS.length) % BUILD_IDS.length], { focus: true });
  });
}

// --- Новости ---

let newsPosts = [];
let releaseNotes = [];
let homeSignatures = [];
let freshPostIds = [];

/** Перерисовываются только изменившиеся карточки: новая запись или новый релиз. */
function renderNewsGrid() {
  const cards = homeCards(newsPosts, releaseNotes, freshPostIds);
  const signatures = cards.map((card) => card.signature);
  const grid = byId('news-grid');
  for (const index of changedSlots(homeSignatures, signatures)) {
    const template = document.createElement('template');
    template.innerHTML = cards[index].html.trim();
    const card = template.content.firstElementChild;
    const existing = grid.children[index];
    if (existing) existing.replaceWith(card);
    else grid.append(card);
  }
  homeSignatures = signatures;
}

function releasesAreFresh() {
  return readList(RELEASES_CACHE_KEY).length > 0 && Date.now() - readNumber(RELEASES_FETCHED_KEY) < RELEASES_REFRESH_MS;
}

function showCachedNews() {
  newsPosts = readList(NEWS_CACHE_KEY);
  releaseNotes = readList(RELEASES_CACHE_KEY);
  renderNewsGrid();
}

let newsRequest = null;
let lastNewsRefresh = 0;

/** Одновременные запросы (таймер, возврат в окно, кнопка) сливаются в один. */
function refreshNews() {
  newsRequest ??= loadLatestNews().finally(() => {
    newsRequest = null;
    lastNewsRefresh = Date.now();
  });
  return newsRequest;
}

async function loadLatestNews() {
  const previousIds = newsPosts.map((post) => post.id).join(',');
  const [news, releases] = await Promise.allSettled([
    invoke('get_vk_news'),
    // У GitHub строгий лимит запросов без авторизации, поэтому история обновляется редко.
    releasesAreFresh() ? Promise.resolve(null) : invoke('get_launcher_releases'),
  ]);
  if (news.status === 'fulfilled' && news.value?.success && news.value.posts?.length > 0) {
    const posts = news.value.posts;
    freshPostIds = [...new Set([...freshPostIds, ...unseenPostIds(posts, readList(NEWS_SEEN_KEY))])];
    writeList(NEWS_SEEN_KEY, posts.map((post) => post.id));
    newsPosts = posts;
    writeList(NEWS_CACHE_KEY, newsPosts);
  }
  if (releases.status === 'fulfilled' && releases.value?.length > 0) {
    releaseNotes = releases.value;
    writeList(RELEASES_CACHE_KEY, releaseNotes);
    writeNumber(RELEASES_FETCHED_KEY, Date.now());
  }
  renderNewsGrid();
  // Открытое окно новостей получает новую запись сразу, прокрутка сохраняется.
  if (openModalKind() === 'news' && newsPosts.map((post) => post.id).join(',') !== previousIds) {
    replaceModalBody(newsArticlesHtml(newsPosts));
  }
}

// --- Стеклянное окно ---

function openNewsModal(index) {
  openGlassModal({ title: 'Новости FenixCraft', html: newsArticlesHtml(newsPosts), kind: 'news' });
  byId(`news-post-${index}`)?.scrollIntoView({ block: 'start' });
  // Открытые новости больше не считаются новыми.
  if (freshPostIds.length > 0) {
    freshPostIds = [];
    renderNewsGrid();
  }
}

function openUpdatesModal() {
  openGlassModal({ title: 'Обновления лаунчера', html: releasesHtml(releaseNotes) });
}

function openTwitchModal() {
  // Twitch требует указать адрес встраивающей страницы.
  const parent = encodeURIComponent(location.hostname);
  openGlassModal({
    title: 'Трансляция BloodRayT',
    wide: true,
    html: twitchHtml(parent),
  });
}

// --- Серверы ---

function activeServers() {
  const custom = currentConfig.customServers;
  return Array.isArray(custom) && custom.length > 0 ? custom : SERVER_DEFAULTS;
}

let lastStatuses = null;

function renderServers(statusById) {
  byId('servers-list').innerHTML = serversHtml(activeServers(), statusById, currentConfig.selectedServer);
  renderAutoJoin();
}

async function refreshServers() {
  let statuses = {};
  try {
    statuses = (await invoke('get_servers_status', { servers: activeServers() })) || {};
  } catch {
    // Ошибка опроса выглядит как недоступные серверы.
  }
  lastStatuses = statuses;
  renderServers(statuses);
}

function selectedServerItem() {
  return activeServers().find((server) => server.ip === currentConfig.selectedServer);
}

/** Подпись под «Играть»: на какой сервер игра зайдёт сама. */
function renderAutoJoin() {
  const server = selectedServerItem();
  const state = autoJoinState({
    server,
    buildVersion: selectedBuild().version,
    enabled: currentConfig.autoJoin !== false,
    // До первого опроса статус неизвестен, поэтому сервер считается доступным.
    online: !lastStatuses || !server || Boolean(lastStatuses[server.id]?.online),
  });
  const chip = byId('autojoin-chip');
  chip.setAttribute('aria-pressed', String(state.enabled));
  byId('autojoin-dot').classList.toggle('is-on', state.enabled && !state.warning);
  byId('autojoin-dot').classList.toggle('is-warn', state.enabled && Boolean(state.warning));
  byId('autojoin-text').innerHTML = state.enabled
    ? `Сразу на сервер <strong>${escapeHtml(state.serverName)}</strong>${state.warning ? ` <span class="autojoin-warning">· ${escapeHtml(state.warning)}</span>` : ''}`
    : 'Автовход выключен: игра откроет главное меню';
  byId('cfg-autojoin').checked = currentConfig.autoJoin !== false;
}

async function copyServerAddress(button) {
  try {
    await navigator.clipboard.writeText(button.dataset.copyIp);
    button.textContent = 'Скопировано';
  } catch {
    button.textContent = 'Ошибка';
  }
  setTimeout(() => { button.textContent = 'IP'; }, 1500);
}

// --- Настройки ---

let saveTimer = null;
let flushSave = () => Promise.resolve();

function bindSettings() {
  const fields = {
    ram: byId('cfg-ram'), width: byId('cfg-width'), height: byId('cfg-height'),
    java: byId('cfg-java'), args: byId('cfg-args'), console: byId('cfg-console'), debug: byId('cfg-debug'),
    autoJoin: byId('cfg-autojoin'), customArgs: byId('cfg-custom-args'), gameArgs: byId('cfg-game-args'),
  };
  fields.ram.value = currentConfig.ram;
  byId('ram-label').textContent = `${currentConfig.ram} МБ`;
  fields.width.value = currentConfig.width;
  fields.height.value = currentConfig.height;
  fields.java.value = currentConfig.javaPath;
  fields.args.value = currentConfig.jvmArgs;
  fields.gameArgs.value = currentConfig.gameArgs ?? '';
  fields.customArgs.checked = currentConfig.customArgsEnabled !== false;
  fields.console.checked = currentConfig.showConsole;
  fields.debug.checked = currentConfig.debugMode;
  const showArgs = () => renderCustomArgs({
    enabled: fields.customArgs.checked, jvmArgs: fields.args.value, gameArgs: fields.gameArgs.value,
  });
  showArgs();
  fields.customArgs.addEventListener('change', showArgs);
  byId('btn-reset-args').addEventListener('click', () => {
    fields.args.value = DEFAULT_JVM_ARGS;
    fields.args.dispatchEvent(new Event('input'));
  });
  byId('cfg-show-modified').checked = readShowModified();
  byId('cfg-show-modified').addEventListener('change', (event) => {
    writeShowModified(event.target.checked);
    versionPicker?.refresh();
  });

  const collect = () => ({
    ram: Number.parseInt(fields.ram.value, 10) || currentConfig.ram,
    width: Number.parseInt(fields.width.value, 10) || currentConfig.width,
    height: Number.parseInt(fields.height.value, 10) || currentConfig.height,
    javaPath: fields.java.value.trim(),
    jvmArgs: fields.args.value.trim(),
    gameArgs: fields.gameArgs.value.trim(),
    customArgsEnabled: fields.customArgs.checked,
    showConsole: fields.console.checked,
    debugMode: fields.debug.checked,
    autoJoin: fields.autoJoin.checked,
  });

  const save = async () => {
    saveTimer = null;
    await persistConfig(collect());
    renderAutoJoin();
  };
  flushSave = () => {
    if (saveTimer === null) return Promise.resolve();
    clearTimeout(saveTimer);
    return save();
  };

  const scheduleSave = () => {
    byId('ram-label').textContent = `${fields.ram.value} МБ`;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(save, SAVE_DEBOUNCE_MS);
  };
  for (const field of Object.values(fields)) field.addEventListener('input', scheduleSave);

  byId('btn-browse-java').addEventListener('click', async () => {
    if (!tauriDialog?.open) return;
    const selected = await tauriDialog.open({ multiple: false, directory: false, title: 'Выберите Java (java / javaw.exe)' });
    if (selected) {
      fields.java.value = selected;
      scheduleSave();
    }
  });
  bindGameDir();
}

async function refreshGameDir() {
  try {
    byId('cfg-game-dir').value = (await invoke('get_game_dir')) || '';
  } catch {
    byId('cfg-game-dir').value = '';
  }
  byId('btn-reset-game-dir').classList.toggle('hidden', !currentConfig.gameDirPath);
}

async function applyGameDir(path) {
  await flushSave();
  // Смена папки идёт в той же очереди, что и сохранения, иначе параллельное сохранение вернуло бы старый путь.
  pendingSave = pendingSave.then(async () => {
    try {
      currentConfig = await invoke('set_game_dir', { path });
    } catch (error) {
      showError({ title: 'Папка не изменена', message: String(error), hint: 'Выберите папку, в которую можно записывать файлы, например на другом диске.' });
    }
    return currentConfig;
  });
  await pendingSave;
  await refreshGameDir();
}

function openGameDir() {
  invoke('open_game_dir').catch((error) => showError({ title: 'Папка не открылась', message: String(error), hint: '' }));
}

function bindGameDir() {
  byId('btn-change-game-dir').addEventListener('click', async () => {
    if (!tauriDialog?.open) return;
    const selected = await tauriDialog.open({ directory: true, multiple: false, title: 'Папка для файлов игры', defaultPath: byId('cfg-game-dir').value || undefined });
    if (selected) await applyGameDir(selected);
  });
  byId('btn-reset-game-dir').addEventListener('click', () => applyGameDir(''));
  byId('btn-open-game-dir').addEventListener('click', openGameDir);
  byId('btn-hero-folder').addEventListener('click', openGameDir);
}

// --- Персонаж ---

let skinStage = null;

let skinCatalog = null;
/** Скин и плащ игрока на Ely.by (data URL), пока он вошёл через Ely.by. */
let elyTextures = null;

async function loadElyTextures(force = false) {
  if (!isEly()) return null;
  if (!elyTextures || force) {
    elyTextures = await invoke('get_ely_textures').catch(() => ({ skin: null, cape: null }));
  }
  return elyTextures;
}

/** Адрес текстуры из каталога сервера скинов: из загруженного списка или по адресу бекенда. */
function catalogTexture(kind, id) {
  if (!id) return '';
  return skinCatalog?.[kind]?.find((item) => item.id === id)?.url ?? '';
}

/** Картинки каталога приходят из лаунчера как data URL, поэтому без каталога выбранный скин не показать. */
async function ensureSkinCatalog() {
  if (skinCatalog || !(currentConfig.skinChoice || currentConfig.capeChoice)) return;
  skinCatalog = await invoke('get_skin_catalog').catch(() => null);
}

async function applySkin() {
  await ensureSkinCatalog();
  byId('btn-skin-reset').classList.toggle('hidden', !currentConfig.skinData);
  if (!skinStage) return;
  // Свой PNG виден только в лаунчере, поэтому он важнее скина, выбранного на сервере.
  // Без выбора на сервере игра покажет скин с Ely.by, и лаунчер показывает его же.
  const ely = currentConfig.skinChoice || currentConfig.capeChoice ? null : await loadElyTextures();
  const skin = currentConfig.skinData || catalogTexture('skins', currentConfig.skinChoice) || ely?.skin || DEFAULT_SKIN;
  try {
    await skinStage.setSkin(skin);
  } catch {
    await skinStage.setSkin(DEFAULT_SKIN).catch(() => {});
  }
  const cape = catalogTexture('capes', currentConfig.capeChoice) || ely?.cape || null;
  await skinStage.setCape(cape).catch(() => {});
}

function openSkinPicker() {
  openSkinCatalog({
    load: async () => {
      skinCatalog = await invoke('get_skin_catalog');
      return skinCatalog;
    },
    current: { skin: currentConfig.skinChoice || '', cape: currentConfig.capeChoice || '' },
    ely: {
      signedIn: isEly(),
      load: async (force) => ({ ...(await loadElyTextures(force)), username: currentConfig.elyUsername }),
    },
    apply: async ({ skin, cape }) => {
      await flushSave();
      await pendingSave;
      currentConfig = await invoke('select_skin', { skin: skin || null, cape: cape || null });
      // Выбранный скин сервера заменяет свой PNG и в лаунчере, чтобы игрок видел то же, что в игре.
      if (currentConfig.skinData) await persistConfig({ skinData: '' });
      await applySkin();
    },
  });
}

function bindSkin() {
  skinStage = createSkinStage({ canvas: byId('skin-canvas'), fallback: byId('skin-fallback') });
  skinStage?.setName(displayName());
  applySkin();

  document.querySelectorAll('[data-skin-pose]').forEach((button) => {
    button.addEventListener('click', () => {
      document.querySelectorAll('[data-skin-pose]').forEach((other) => other.setAttribute('aria-pressed', String(other === button)));
      skinStage?.setPose(button.dataset.skinPose);
    });
  });

  const input = byId('skin-file-input');
  byId('btn-skin-change').addEventListener('click', () => input.click());
  byId('btn-skin-pick').addEventListener('click', openSkinPicker);
  input.addEventListener('change', async () => {
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    if (file.size > MAX_SKIN_BYTES) {
      showError({ title: 'Скин не загружен', message: 'Файл больше 32 КБ.', hint: 'Скин Minecraft — это PNG 64×64, обычно он весит несколько килобайт.' });
      return;
    }
    try {
      const skinData = await readSkinFile(file);
      await persistConfig({ skinData });
      await applySkin();
    } catch (error) {
      showError({ title: 'Скин не загружен', message: error.message, hint: 'Скачайте скин в формате PNG, например на сайте со скинами.' });
    }
  });
  byId('btn-skin-reset').addEventListener('click', async () => {
    await persistConfig({ skinData: '' });
    await applySkin();
  });
}

// --- Профиль ---

let accountTab = 'offline';
let nickTimer = null;

const NICK_RULES_MESSAGE = 'Ник: 3–16 латинских букв, цифр или _';

function isEly() {
  return isElySignedIn(currentConfig);
}

function displayName() {
  return isEly() ? currentConfig.elyUsername : currentConfig.username;
}

function renderProfile() {
  document.querySelectorAll('[data-account-tab]').forEach((tab) => {
    tab.setAttribute('aria-pressed', String(tab.dataset.accountTab === accountTab));
  });
  byId('sidebar-offline-box').classList.toggle('hidden', accountTab !== 'offline');
  byId('sidebar-ely-box').classList.toggle('hidden', accountTab !== 'ely');
  renderElyBox(currentConfig);
  byId('sidebar-account-type').textContent = isEly() ? 'Ely.by' : 'Пиратка';
  byId('sidebar-username').textContent = displayName() || 'Новый игрок';
  const nickInput = byId('sidebar-nick-input');
  if (document.activeElement !== nickInput) nickInput.value = currentConfig.username || '';
  skinStage?.setName(displayName());
}

function flashAuthMessage(text) {
  const message = byId('sidebar-auth-msg');
  message.textContent = text;
  setTimeout(() => {
    if (message.textContent === text) message.textContent = '';
  }, 4000);
}

function bindProfile() {
  bindAvatar({ current: currentConfig.avatarData, save: (avatarData) => persistConfig({ avatarData }), notify: flashAuthMessage });
  document.querySelectorAll('[data-account-tab]').forEach((tab) => {
    tab.addEventListener('click', () => selectAccountTab(tab.dataset.accountTab));
  });

  // Ник применяется сразу, как только он становится допустимым.
  const nickInput = byId('sidebar-nick-input');
  nickInput.addEventListener('input', () => {
    clearTimeout(nickTimer);
    const username = nickInput.value.trim();
    if (!isValidUsername(username)) {
      byId('sidebar-auth-msg').textContent = NICK_RULES_MESSAGE;
      return;
    }
    byId('sidebar-auth-msg').textContent = '';
    nickTimer = setTimeout(async () => {
      await persistConfig({ username, accountType: 'offline' });
      refreshProfileViews();
    }, SAVE_DEBOUNCE_MS);
  });

  bindElyAccount({
    invoke,
    notify: flashAuthMessage,
    onChange: async (config) => {
      currentConfig = config;
      accountTab = isEly() ? 'ely' : accountTab;
      elyTextures = null;
      refreshProfileViews();
      await applySkin();
    },
  });
}

function refreshProfileViews() {
  renderProfile();
  byId('shop-grid').innerHTML = shopGridHtml(displayName());
}

/** Вкладка «Пиратка» переключает игру на ник, вкладка «Ely.by» — на вход Ely.by, если он уже выполнен. */
async function selectAccountTab(tab) {
  accountTab = tab;
  byId('sidebar-auth-msg').textContent = '';
  if (tab === 'offline' && isEly()) {
    await persistConfig({ accountType: 'offline' });
  } else if (tab === 'ely' && currentConfig.elyUuid && !isEly()) {
    await persistConfig({ accountType: 'ely' });
  }
  refreshProfileViews();
  await applySkin();
}

// --- Запуск игры ---

function setPlayState(text, disabled) {
  byId('btn-play').disabled = disabled;
  byId('btn-play-text').textContent = text;
}

function resetPlayState() {
  setPlayState('Играть', false);
  byId('progress-container').classList.add('hidden');
}

async function startGame() {
  await flushSave();
  await pendingSave;
  setPlayState('Подготовка…', true);
  byId('progress-status').textContent = 'Проверка файлов…';
  byId('progress-percent').textContent = '';
  byId('progress-bar').style.width = '0%';
  byId('progress-container').classList.remove('hidden');
  if (currentConfig.showConsole) switchTab('console');
  try {
    await invoke('launch_game');
  } catch (error) {
    showError(error);
    if (error?.code !== 'already_running') resetPlayState();
    return;
  }
  // В браузере нет процесса игры, поэтому события завершения не придут.
  if (!tauriCore) resetPlayState();
}

function bindGameEvents() {
  if (!tauriEvent) return;
  tauriEvent.listen('game:log', (event) => appendConsole(event.payload));
  tauriEvent.listen('game:progress', ({ payload }) => {
    const percent = payload.total > 0 ? Math.round((payload.task / payload.total) * 100) : 0;
    byId('progress-status').textContent = `${payload.type} (${payload.task}/${payload.total})`;
    byId('progress-percent').textContent = `${percent}%`;
    byId('progress-bar').style.width = `${percent}%`;
  });
  tauriEvent.listen('game:running', () => {
    setPlayState('Игра запущена', true);
    byId('progress-container').classList.add('hidden');
  });
  tauriEvent.listen('game:error', (event) => showError(event.payload));
  tauriEvent.listen('game:closed', resetPlayState);
}

// --- Статус сети, Twitch и обновления ---

const CONNECTIVITY_TONES = {
  online: { dot: 'bg-emerald-400', text: 'text-emerald-400' },
  degraded: { dot: 'bg-amber-400', text: 'text-amber-400' },
  offline: { dot: 'bg-slate-500', text: 'text-slate-400' },
};

let wasInternetAvailable = true;

async function refreshConnectivity() {
  let state = { internet: false, backend: false };
  try {
    state = await invoke('get_connectivity');
  } catch {
    // Ошибка проверки показывается как отсутствие сети.
  }
  const label = connectivityLabel(state);
  const tone = CONNECTIVITY_TONES[label.tone];
  const pill = byId('network-status');
  pill.className = `flex items-center gap-1.5 text-[11px] font-medium pr-2 ${tone.text}`;
  pill.title = label.hint;
  byId('network-dot').className = `w-1.5 h-1.5 rounded-full ${tone.dot}`;
  byId('network-text').textContent = label.text;
  if (state.internet && !wasInternetAvailable) {
    refreshNews();
    refreshServers();
  }
  wasInternetAvailable = state.internet;
}

async function updateTwitchStatus() {
  let live = false;
  try {
    live = Boolean((await invoke('check_twitch_live'))?.live);
  } catch {
    // Статус стрима необязателен.
  }
  byId('btn-twitch-nav').classList.toggle('twitch-live-active', live);
  byId('twitch-live-badge').classList.toggle('hidden', !live);
}

// --- Общие обработчики ---

function bindWindowControls() {
  const commands = { 'btn-win-min': 'window_minimize', 'btn-win-close': 'window_close' };
  for (const [id, command] of Object.entries(commands)) {
    byId(id).addEventListener('click', () => invoke(command));
  }
}

function activateElement(target) {
  const external = target.closest('[data-external]');
  if (external) {
    openUrl(external.dataset.external);
    return;
  }
  const buy = target.closest('[data-buy]');
  if (buy) {
    const nickname = encodeURIComponent(displayName() || 'Player');
    openUrl(`${SHOP_URL}?nickname=${nickname}&item=${encodeURIComponent(buy.dataset.buy)}`);
    return;
  }
  const build = target.closest('[data-build]');
  if (build) {
    selectBuild(build.dataset.build);
    return;
  }
  const copyIp = target.closest('[data-copy-ip]');
  if (copyIp) {
    copyServerAddress(copyIp);
    return;
  }
  const server = target.closest('[data-server-ip]');
  if (server) {
    persistConfig({ selectedServer: server.dataset.serverIp }).then(() => renderServers(lastStatuses ?? {}));
    return;
  }
  if (target.closest('#autojoin-chip')) {
    persistConfig({ autoJoin: currentConfig.autoJoin === false }).then(renderAutoJoin);
    return;
  }
  const newsCard = target.closest('[data-open-news]');
  if (newsCard) {
    openNewsModal(Number(newsCard.dataset.openNews));
    return;
  }
  if (target.closest('[data-open-updates]')) {
    openUpdatesModal();
    return;
  }
  const tabLink = target.closest('[data-open-tab]');
  if (tabLink) switchTab(tabLink.dataset.openTab);
}

function bindDelegatedActions() {
  document.addEventListener('click', (event) => activateElement(event.target));
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      // Закрывается только верхнее окно.
      if (byId('error-modal').classList.contains('hidden')) closeGlassModal();
      else hideError();
    }
    const isRoleButton = event.target.getAttribute?.('role') === 'button';
    if (isRoleButton && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      if (event.target.id === 'avatar-container') byId('avatar-file-input').click();
      else activateElement(event.target);
    }
  });
  // Если картинка новости не загрузилась, показываем локальный баннер.
  document.addEventListener('error', (event) => {
    const image = event.target;
    if (image.classList?.contains('news-image') && !image.src.endsWith(LOCAL_NEWS_IMAGES[0])) {
      image.src = LOCAL_NEWS_IMAGES[0];
    }
  }, true);

  document.querySelectorAll('.nav-btn[data-tab]').forEach((button) => {
    button.addEventListener('click', () => switchTab(button.dataset.tab));
  });
  byId('btn-play').addEventListener('click', startGame);
  byId('btn-twitch-nav').addEventListener('click', openTwitchModal);
  byId('btn-glass-close').addEventListener('click', closeGlassModal);
  byId('glass-modal').addEventListener('click', (event) => {
    if (event.target === byId('glass-modal')) closeGlassModal();
  });
  // Плеер разворачивается внутри окна, поэтому на весь экран нужно перевести и само окно.
  document.addEventListener('fullscreenchange', () => {
    invoke('window_set_fullscreen', { fullscreen: Boolean(document.fullscreenElement) }).catch(() => {});
  });
  byId('btn-refresh-news').addEventListener('click', refreshNews);
  byId('btn-ping-servers').addEventListener('click', refreshServers);
  byId('btn-error-close').addEventListener('click', hideError);
  byId('btn-error-console').addEventListener('click', () => {
    hideError();
    switchTab('console');
  });
}

// --- Любая версия ---

let versionPicker = null;

async function loadReleases() {
  const list = await invoke('list_minecraft_releases');
  if (list?.latest && list.latest !== latestRelease) {
    latestRelease = list.latest;
    renderBuilds();
    renderHero(false);
  }
  return { releases: list?.releases ?? [], offline: Boolean(list?.offline) };
}

function bindVersions() {
  versionPicker = bindVersionPicker({
    loadReleases,
    showModified: readShowModified,
    selectedId: () => currentConfig.selectedVersion,
    onSelect: (id) => selectBuild(id),
  });
  // Номер последней версии нужен на карточке Vanilla сразу, без открытия списка.
  loadReleases().catch(() => {});
}

// --- Производительность ---

let closeTimer = null;

/** «Выключить все анимации» и «Закрыть лаунчер после запуска игры». */
function bindPerformance({ wallpaper, embers }) {
  const motionBox = byId('cfg-no-motion');
  const closeBox = byId('cfg-close-after-launch');
  const apply = () => {
    applyMotion(motionBox.checked, { wallpaper, heroVideo, embers, skinStage });
    byId('cfg-live-bg').disabled = motionBox.checked;
  };
  motionBox.checked = isMotionOff();
  motionBox.addEventListener('change', () => {
    setMotionOff(motionBox.checked);
    apply();
  });
  apply();

  closeBox.checked = isCloseAfterLaunch();
  closeBox.addEventListener('change', () => {
    setCloseAfterLaunch(closeBox.checked);
    if (!closeBox.checked) clearTimeout(closeTimer);
  });
  // Игра работает отдельным процессом, поэтому закрытие лаунчера её не останавливает.
  tauriEvent?.listen('game:running', () => {
    if (!isCloseAfterLaunch()) return;
    byId('btn-play-text').textContent = 'Игра запущена · лаунчер закроется';
    closeTimer = setTimeout(() => {
      // Галочку могли снять, пока шли 5 секунд.
      if (isCloseAfterLaunch()) invoke('window_close');
    }, CLOSE_AFTER_LAUNCH_MS);
  });
  // Если игра сразу вылетела или закрылась, лаунчер остаётся, чтобы показать причину.
  const cancelClose = () => clearTimeout(closeTimer);
  tauriEvent?.listen('game:error', cancelClose);
  tauriEvent?.listen('game:closed', cancelClose);
}

async function showLauncherVersion() {
  const version = await invoke('get_launcher_version');
  byId('app-version-badge').textContent = `v${version}`;
}

async function start() {
  fitPreview();
  const wallpaper = startWallpaper(byId('bg-wallpaper'));
  byId('cfg-live-bg').checked = isLiveWallpaperOn();
  byId('cfg-live-bg').addEventListener('change', (event) => wallpaper.setLive(event.target.checked));
  const embers = startEmbers(byId('bg-embers'));
  bindWindowControls();
  bindDelegatedActions();
  bindConsole();
  bindGameEvents();
  bindBuildKeys();

  currentConfig = await invoke('get_config');
  accountTab = isEly() ? 'ely' : 'offline';
  renderBuilds();
  renderHero(false);
  bindSettings();
  heroVideo = bindHeroVideo({ hero: document.querySelector('.hero'), video: byId('hero-video'), chip: byId('hero-anim-chip'), dot: byId('hero-anim-dot') });
  heroVideo.update(selectedBuild());
  bindVersions();
  bindProfile();
  bindSkin();
  bindPerformance({ wallpaper, embers });
  refreshProfileViews();
  renderServers({});
  showCachedNews();

  // Интерфейс уже готов: сетевые данные догружаются в фоне и не задерживают запуск.
  showLauncherVersion();
  refreshGameDir();
  refreshNews();
  refreshServers();
  refreshConnectivity();
  updateTwitchStatus();
  checkForUpdates();
  setInterval(refreshServers, SERVERS_REFRESH_MS);
  setInterval(updateTwitchStatus, REFRESH_INTERVAL_MS);
  setInterval(refreshConnectivity, CONNECTIVITY_INTERVAL_MS);
  setInterval(refreshNews, NEWS_REFRESH_MS);
  // Вернувшись в лаунчер, игрок сразу видит свежие записи.
  document.addEventListener('visibilitychange', () => {
    if (!document.hidden && Date.now() - lastNewsRefresh > NEWS_FOCUS_REFRESH_MS) refreshNews();
  });
}

start().catch((error) => showError({ title: 'Лаунчер запустился с ошибкой', message: String(error), hint: 'Перезапустите лаунчер.' }));
