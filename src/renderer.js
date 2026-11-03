import {
  connectivityLabel,
  escapeHtml,
  isValidUsername,
  safeImageUrl,
  safeLinkUrl,
  serverStatusLabel,
} from './utils.js';

const tauriCore = window.__TAURI__?.core;
const tauriEvent = window.__TAURI__?.event;
const tauriDialog = window.__TAURI__?.dialog;

const LOCAL_NEWS_IMAGE = 'baner.webp';
const NEWS_CACHE_KEY = 'fenix.news.v1';
const SHOP_URL = 'https://fenixcraft.trademc.org/';
const TWITCH_URL = 'https://www.twitch.tv/bloodrayt';
const REFRESH_INTERVAL_MS = 30000;
const CONNECTIVITY_INTERVAL_MS = 60000;
const SAVE_DEBOUNCE_MS = 300;
const MAX_CONSOLE_LINES = 5000;
const MAX_AVATAR_BYTES = 2 * 1024 * 1024;
const AVATAR_SIZE = 128;
const VALID_PROFILES = ['latest-vanilla', '1.21.11-vanilla', '1.21.1-neoforge', '1.21.11-fabric', '1.21.11-iris', '1.21.1-iris-neoforge', '1.21.11-iris-fabric'];
const TABS = ['home', 'shop', 'servers', 'settings', 'faq', 'console'];

/** Вне приложения (проверка верстки в браузере) вызовы заменяются заглушками. */
const previewResponses = {
  get_config: () => ({
    username: 'Preview_Player', accountType: 'offline', ram: 6144, width: 1280, height: 720,
    javaPath: '', jvmArgs: '-XX:+UseG1GC', showConsole: false, debugMode: false,
    selectedVersion: 'latest-vanilla', selectedServer: 'play.fenixcraft.ru',
    customServers: [], avatarData: '', licenseLogin: '', licenseToken: '', licenseUuid: '',
  }),
  get_launcher_version: () => 'dev',
  get_connectivity: () => ({ internet: true, backend: true }),
  get_servers_status: () => ({}),
  get_vk_news: () => ({ success: false }),
  check_twitch_live: () => ({ live: false }),
  check_launcher_update: () => ({ updateAvailable: false }),
  save_launcher_config: (args) => args.newConfig,
};

function invoke(command, args) {
  if (tauriCore) return tauriCore.invoke(command, args);
  return Promise.resolve(previewResponses[command]?.(args));
}

const SERVER_DEFAULTS = [
  { id: 'fenix-main', name: 'FenixCraft', desc: 'Выживание • Vanilla/Mods', ip: 'play.fenixcraft.ru', port: 25565, icon: '🦅', maxDefault: 1000 },
  { id: 'fenix-mods', name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', port: 25565, icon: '⚙️', maxDefault: 500 },
  { id: 'fenix-rpg', name: 'FenixRPG', desc: 'Fabric + Iris • 1.21.11', ip: 'rpg.fenixcraft.ru', port: 25565, icon: '⚔️', maxDefault: 500 },
  { id: 'fenix-pvp', name: 'FenixArena', desc: 'Анархия • 1.21.11', ip: 'pvp.fenixcraft.ru', port: 25565, icon: '⭐', maxDefault: 300 },
];

const SHOP_ITEMS = [
  {
    id: 'vip', name: 'Привилегия [VIP]', price: '149 ₽', badge: 'Хит продаж', icon: '👑',
    color: 'from-amber-500/20 to-yellow-600/5 border-amber-500/40',
    btnColor: 'bg-amber-500 hover:bg-amber-400 text-slate-950',
    perks: ['Префикс [VIP] в чате и табе', 'Полет /fly в лобби и приватных зонах', 'Доступ к киту /kit vip каждые 24ч', 'Вход на переполненный сервер', '5 дополнительных точек дома /sethome'],
  },
  {
    id: 'creativ', name: 'Привилегия [Creative]', price: '299 ₽', badge: 'Строителям', icon: '🎨',
    color: 'from-cyan-500/20 to-blue-600/5 border-cyan-500/40',
    btnColor: 'bg-cyan-400 hover:bg-cyan-300 text-slate-950',
    perks: ['Режим креатива /gm 1 в разрешенных мирах', 'Доступ к командам WorldEdit (//set, //copy)', 'Префикс [Creative] и цветной чат', 'Эксклюзивный набор /kit creative', 'Все возможности привилегии VIP'],
  },
  {
    id: 'moder', name: 'Привилегия [Moder]', price: '599 ₽', badge: 'Страж порядка', icon: '🛡️',
    color: 'from-purple-500/20 to-indigo-600/5 border-purple-500/40',
    btnColor: 'bg-purple-500 hover:bg-purple-400 text-white',
    perks: ['Команды модерации (/mute, /kick, /tp)', 'Просмотр инвентаря игроков /invsee', 'Режим невидимости /vanish', 'Ежедневный бонус /kit moder', 'Иммунитет к кику и муту'],
  },
  {
    id: 'admin', name: 'Привилегия [Admin]', price: '999 ₽', badge: 'Максимум прав', icon: '⚡',
    color: 'from-rose-500/20 to-red-600/5 border-rose-500/40',
    btnColor: 'bg-rose-500 hover:bg-rose-400 text-white',
    perks: ['Полный набор прав [Admin] на сервере', 'Команды /ban, /tempban, /god, /heal', 'Управление погодой и временем в мире', 'Элитный кит /kit admin с топ-ресурсами', 'Включает все привилегии ниже'],
  },
  {
    id: 'money', name: 'Игровая валюта [Money]', price: 'от 49 ₽', badge: 'Экономика', icon: '💰',
    color: 'from-emerald-500/20 to-teal-600/5 border-emerald-500/40',
    btnColor: 'bg-emerald-500 hover:bg-emerald-400 text-slate-950',
    perks: ['Пакеты: 50 000$, 250 000$, 1 000 000$', 'Мгновенное зачисление на баланс /money', 'Покупка редких ресурсов на аукционе /ah', 'Создание собственных варпов и кланов', 'Бонус +25% при покупке от 250 000$'],
  },
];

const FALLBACK_NEWS = [
  { tag: 'VK.RU/FENIXCRAFT', title: 'Сообщество FenixCraft ВКонтакте', text: 'Нажмите на карточку, чтобы открыть официальную стену vk.ru/fenixcraft со всеми новостями и конкурсами!', date: 'Официальная группа', image: '', url: 'https://vk.ru/fenixcraft' },
  { tag: 'Магазин', tone: 'amber', title: 'Привилегии VIP, Creative, Moder, Admin и Money', text: 'Поддержите развитие сервера и получите уникальные возможности в разделе «Магазин».', date: 'FenixCraft Shop', image: '', url: SHOP_URL },
  { tag: 'Лаунчер', tone: 'cyan', title: 'Игра запускается и без интернета', text: 'После первой полной загрузки сборка запускается без подключения к сети. Файлы проверяются по контрольным суммам.', date: 'Обновление лаунчера', image: '', url: 'https://vk.ru/fenixcraft' },
];

let currentConfig = {};
const byId = (id) => document.getElementById(id);

function openUrl(url) {
  const safe = safeLinkUrl(url);
  if (!safe) return Promise.resolve();
  if (tauriCore) {
    return invoke('open_external_url', { url: safe }).catch((error) => showError({ title: 'Ссылка не открылась', message: String(error), hint: '' }));
  }
  window.open(safe, '_blank', 'noopener');
  return Promise.resolve();
}

let pendingSave = Promise.resolve();

/** Сохранения идут строго по очереди, иначе одновременные изменения затирали бы друг друга. */
function persistConfig(changes) {
  pendingSave = pendingSave.then(async () => {
    try {
      currentConfig = await invoke('save_launcher_config', { newConfig: { ...currentConfig, ...changes } });
    } catch (error) {
      showError({ title: 'Настройки не сохранены', message: String(error), hint: 'Проверьте свободное место на диске.' });
    }
    return currentConfig;
  });
  return pendingSave;
}

// --- Диалог ошибки ---

function showError({ title, message, hint }) {
  byId('error-title').textContent = title;
  byId('error-message').textContent = message;
  byId('error-hint').textContent = hint || '';
  byId('error-modal').classList.remove('hidden');
  byId('btn-error-close').focus();
}

function hideError() {
  byId('error-modal').classList.add('hidden');
}

// --- Вкладки ---

function switchTab(tabId) {
  for (const id of TABS) byId(`tab-${id}`)?.classList.add('hidden');
  (byId(`tab-${tabId}`) || byId('tab-home')).classList.remove('hidden');
  document.querySelectorAll('.nav-btn').forEach((button) => {
    button.classList.toggle('active-nav', button.dataset.tab === tabId);
  });
}

// --- Магазин ---

function renderShop() {
  const buyer = escapeHtml(currentConfig.username || 'Игрока');
  byId('shop-grid').innerHTML = SHOP_ITEMS.map((item) => `
    <div class="rounded-2xl p-4 bg-gradient-to-br ${item.color} border flex flex-col justify-between space-y-4">
      <div>
        <div class="flex items-center justify-between">
          <span class="text-2xl">${item.icon}</span>
          <span class="text-[10px] font-bold uppercase tracking-wider px-2.5 py-0.5 rounded-full bg-white/10 text-white">${item.badge}</span>
        </div>
        <h3 class="text-base font-extrabold text-white mt-2">${item.name}</h3>
        <div class="text-xl font-black text-emerald-400 mt-0.5">${item.price}</div>
        <ul class="text-xs text-slate-300 space-y-1.5 mt-3">
          ${item.perks.map((perk) => `<li class="flex items-start space-x-1.5"><span class="text-emerald-400">✔</span><span>${perk}</span></li>`).join('')}
        </ul>
      </div>
      <button data-buy="${item.id}" class="w-full py-2.5 rounded-xl font-bold text-xs transition active:scale-95 ${item.btnColor}">Купить для ${buyer}</button>
    </div>
  `).join('');
}

// --- Новости ---

/** Классы выбираются только из этого списка: данные новостей приходят извне. */
const NEWS_TAG_CLASSES = new Map([
  ['amber', 'bg-amber-500 text-slate-950'],
  ['cyan', 'bg-cyan-500 text-slate-950'],
]);

function newsTagClass(post) {
  return NEWS_TAG_CLASSES.get(post.tone) ?? 'bg-emerald-500 text-slate-950';
}

function renderNews(posts) {
  byId('news-grid').innerHTML = posts.slice(0, 3).map((post) => `
    <div role="button" tabindex="0" data-external="${escapeHtml(safeLinkUrl(post.url) || 'https://vk.ru/fenixcraft')}"
         class="glass-card rounded-xl overflow-hidden flex flex-col justify-between border border-white/5 hover:border-emerald-500/40 transition cursor-pointer">
      <div class="h-28 relative bg-slate-900">
        <img src="${escapeHtml(safeImageUrl(post.image, LOCAL_NEWS_IMAGE))}" alt="" loading="lazy" class="news-image w-full h-full object-cover" />
        <span class="absolute bottom-2 left-2 text-[10px] font-bold px-2.5 py-0.5 rounded-full ${newsTagClass(post)}">${escapeHtml(post.tag || 'VK.RU/FENIXCRAFT')}</span>
      </div>
      <div class="p-3.5 flex-1 flex flex-col justify-between space-y-2">
        <div>
          <h4 class="text-sm font-bold text-white line-clamp-1">${escapeHtml(post.title || 'Новость FenixCraft')}</h4>
          <p class="text-xs text-slate-400 mt-1 line-clamp-2">${escapeHtml(post.text)}</p>
        </div>
        <div class="text-[11px] text-emerald-400/80 pt-1 flex justify-between items-center">
          <span>📅 ${escapeHtml(post.date)}</span>
          <span>Открыть ↗</span>
        </div>
      </div>
    </div>
  `).join('');
}

function readCachedNews() {
  try {
    const cached = JSON.parse(localStorage.getItem(NEWS_CACHE_KEY));
    return Array.isArray(cached) && cached.length > 0 ? cached : null;
  } catch {
    return null;
  }
}

function writeCachedNews(posts) {
  try {
    localStorage.setItem(NEWS_CACHE_KEY, JSON.stringify(posts));
  } catch {
    // Кеш новостей необязателен: без него покажутся карточки по умолчанию.
  }
}

async function fetchNews() {
  const response = await invoke('get_vk_news');
  if (!response?.success) return null;
  if (Array.isArray(response.posts) && response.posts.length > 0) return response.posts;
  return null;
}

async function loadNews() {
  renderNews(readCachedNews() || FALLBACK_NEWS);
  try {
    const posts = await fetchNews();
    if (posts) {
      writeCachedNews(posts);
      renderNews(posts);
    }
  } catch {
    // Без сети остаются сохранённые новости.
  }
}

// --- Серверы ---

function activeServers() {
  const custom = currentConfig.customServers;
  return Array.isArray(custom) && custom.length > 0 ? custom : SERVER_DEFAULTS;
}

function renderServers(statusById) {
  const html = activeServers().map((server) => {
    const status = statusById[server.id];
    const selected = currentConfig.selectedServer === server.ip;
    return `
      <div role="button" tabindex="0" data-server-ip="${escapeHtml(server.ip)}"
           class="glass-card p-3 rounded-xl flex items-center justify-between cursor-pointer transition border ${selected ? 'border-emerald-500 bg-emerald-500/10' : 'border-white/5 hover:border-white/20'}">
        <div class="flex items-center space-x-3 min-w-0">
          <div class="w-9 h-9 rounded-full bg-slate-800 flex items-center justify-center text-base shrink-0">${escapeHtml(server.icon)}</div>
          <div class="min-w-0">
            <div class="text-xs font-bold text-white truncate">${escapeHtml(server.name)}</div>
            <div class="text-[10px] text-slate-400 truncate">${escapeHtml(server.desc)} • ${escapeHtml(server.ip)}</div>
          </div>
        </div>
        <div class="text-xs font-semibold flex items-center space-x-1.5 shrink-0 pl-2 ${status?.online ? 'text-emerald-400' : 'text-slate-500'}">
          <span class="w-2 h-2 rounded-full ${status?.online ? 'bg-emerald-400' : 'bg-red-500'}"></span>
          <span>${escapeHtml(serverStatusLabel(status))}</span>
        </div>
      </div>
    `;
  }).join('');
  byId('servers-list').innerHTML = html;
  byId('servers-full-list').innerHTML = html;
}

async function refreshServers() {
  let statuses = {};
  try {
    statuses = (await invoke('get_servers_status', { servers: activeServers() })) || {};
  } catch {
    // Ошибка опроса выглядит как недоступные серверы.
  }
  renderServers(statuses);
}

async function saveCustomServer() {
  const name = byId('new-srv-name').value.trim();
  const ip = byId('new-srv-ip').value.trim();
  if (!name || !ip) return;
  const list = activeServers();
  const existing = list.findIndex((server) => server.ip.toLowerCase() === ip.toLowerCase() || server.name.toLowerCase() === name.toLowerCase());
  const item = {
    id: existing >= 0 ? list[existing].id : `srv-${Date.now()}`,
    name,
    desc: byId('new-srv-desc').value.trim() || 'Сервер FenixCraft',
    ip,
    port: Number.parseInt(byId('new-srv-port').value, 10) || 25565,
    icon: existing >= 0 ? list[existing].icon : '🦅',
    maxDefault: 1000,
  };
  const updated = existing >= 0 ? list.with(existing, item) : [...list, item];
  await persistConfig({ customServers: updated, selectedServer: ip });
  for (const id of ['new-srv-name', 'new-srv-desc', 'new-srv-ip']) byId(id).value = '';
  await refreshServers();
}

// --- Настройки ---

let saveTimer = null;
let flushSave = () => Promise.resolve();

function bindSettings() {
  const fields = {
    username: byId('cfg-username'), ram: byId('cfg-ram'), width: byId('cfg-width'), height: byId('cfg-height'),
    java: byId('cfg-java'), args: byId('cfg-args'), console: byId('cfg-console'), debug: byId('cfg-debug'),
    version: byId('select-version'),
  };
  fields.username.value = currentConfig.username;
  fields.ram.value = currentConfig.ram;
  byId('ram-label').textContent = `${currentConfig.ram} МБ`;
  fields.width.value = currentConfig.width;
  fields.height.value = currentConfig.height;
  fields.java.value = currentConfig.javaPath;
  fields.args.value = currentConfig.jvmArgs;
  fields.console.checked = currentConfig.showConsole;
  fields.debug.checked = currentConfig.debugMode;
  fields.version.value = VALID_PROFILES.includes(currentConfig.selectedVersion) ? currentConfig.selectedVersion : 'latest-vanilla';

  const selectedProfileName = () => fields.version.options[fields.version.selectedIndex]?.text || 'Vanilla';
  byId('last-launch-ver').textContent = selectedProfileName();

  const collect = () => ({
    username: fields.username.value.trim(),
    ram: Number.parseInt(fields.ram.value, 10) || currentConfig.ram,
    width: Number.parseInt(fields.width.value, 10) || currentConfig.width,
    height: Number.parseInt(fields.height.value, 10) || currentConfig.height,
    javaPath: fields.java.value.trim(),
    jvmArgs: fields.args.value.trim(),
    showConsole: fields.console.checked,
    debugMode: fields.debug.checked,
    selectedVersion: fields.version.value,
  });

  const save = async () => {
    saveTimer = null;
    await persistConfig(collect());
    renderProfile();
    renderShop();
    byId('last-launch-ver').textContent = selectedProfileName();
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
}

// --- Профиль ---

function renderProfile() {
  const isEly = currentConfig.accountType === 'mojang';
  byId('sidebar-acc-type').value = isEly ? 'mojang' : 'offline';
  byId('sidebar-offline-box').classList.toggle('hidden', isEly);
  byId('sidebar-license-box').classList.toggle('hidden', !isEly);
  byId('sidebar-account-type').textContent = isEly ? 'Ely.by' : 'Без пароля';
  byId('sidebar-username').textContent = currentConfig.username || 'Новый игрок';
  byId('sidebar-nick-input').value = currentConfig.username || '';
  byId('sidebar-license-login').value = currentConfig.licenseLogin || '';
  const settingsUsername = byId('cfg-username');
  if (document.activeElement !== settingsUsername) settingsUsername.value = currentConfig.username || '';
}

function flashAuthMessage(text) {
  const message = byId('sidebar-auth-msg');
  message.textContent = text;
  setTimeout(() => {
    if (message.textContent === text) message.textContent = '';
  }, 3000);
}

function cropAvatar(image) {
  const canvas = document.createElement('canvas');
  canvas.width = AVATAR_SIZE;
  canvas.height = AVATAR_SIZE;
  const side = Math.min(image.width, image.height);
  canvas.getContext('2d').drawImage(image, (image.width - side) / 2, (image.height - side) / 2, side, side, 0, 0, AVATAR_SIZE, AVATAR_SIZE);
  return canvas.toDataURL('image/webp', 0.85);
}

function bindProfile() {
  const avatarImg = byId('sidebar-avatar-img');
  const avatarInput = byId('avatar-file-input');
  if (currentConfig.avatarData) avatarImg.src = currentConfig.avatarData;

  byId('avatar-container').addEventListener('click', () => avatarInput.click());
  avatarInput.addEventListener('change', (event) => {
    const file = event.target.files?.[0];
    if (!file) return;
    if (file.size > MAX_AVATAR_BYTES) {
      flashAuthMessage('Файл больше 2 МБ');
      return;
    }
    const reader = new FileReader();
    reader.onerror = () => flashAuthMessage('Не удалось открыть картинку');
    reader.onload = () => {
      const image = new Image();
      image.onerror = () => flashAuthMessage('Не удалось открыть картинку');
      image.onload = async () => {
        const avatarData = cropAvatar(image);
        avatarImg.src = avatarData;
        await persistConfig({ avatarData });
        flashAuthMessage('Аватарка сохранена');
      };
      image.src = reader.result;
    };
    reader.readAsDataURL(file);
  });

  byId('sidebar-acc-type').addEventListener('change', (event) => {
    const isEly = event.target.value === 'mojang';
    byId('sidebar-offline-box').classList.toggle('hidden', isEly);
    byId('sidebar-license-box').classList.toggle('hidden', !isEly);
  });

  byId('btn-sidebar-save-nick').addEventListener('click', async () => {
    const username = byId('sidebar-nick-input').value.trim();
    if (!isValidUsername(username)) {
      flashAuthMessage('Ник: 3–16 латинских букв, цифр или _');
      return;
    }
    await persistConfig({ username, accountType: 'offline', licenseToken: '', licenseUuid: '' });
    renderProfile();
    renderShop();
    flashAuthMessage(`Ник сохранён: ${username}`);
  });

  const loginButton = byId('btn-sidebar-login-license');
  const passwordInput = byId('sidebar-license-pass');
  loginButton.addEventListener('click', async () => {
    const login = byId('sidebar-license-login').value.trim();
    if (!login || !passwordInput.value) {
      flashAuthMessage('Введите логин и пароль');
      return;
    }
    loginButton.disabled = true;
    loginButton.textContent = 'Подключение...';
    const result = await invoke('authenticate_ely', { username: login, password: passwordInput.value })
      .catch((error) => ({ success: false, error: String(error) }));
    passwordInput.value = '';
    loginButton.disabled = false;
    loginButton.textContent = 'Войти через Ely.by';
    if (!result?.success) {
      flashAuthMessage(result?.error || 'Не удалось войти');
      return;
    }
    await persistConfig({
      username: result.username,
      accountType: 'mojang',
      licenseLogin: login,
      licenseToken: result.token,
      licenseUuid: result.uuid,
    });
    renderProfile();
    renderShop();
    flashAuthMessage(`Вход выполнен: ${result.username}`);
  });
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
  setPlayState('Подготовка...', true);
  byId('progress-status').textContent = 'Проверка файлов...';
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

// --- Консоль ---

let consoleLines = [];
let consoleFlushScheduled = false;

function renderConsole() {
  consoleFlushScheduled = false;
  const output = byId('console-output');
  output.textContent = consoleLines.join('\n');
  output.scrollTop = output.scrollHeight;
}

function appendConsole(line) {
  consoleLines.push(line);
  if (consoleLines.length > MAX_CONSOLE_LINES) consoleLines = consoleLines.slice(-MAX_CONSOLE_LINES);
  if (!consoleFlushScheduled) {
    consoleFlushScheduled = true;
    requestAnimationFrame(renderConsole);
  }
}

function bindConsole() {
  byId('btn-clear-console').addEventListener('click', () => {
    consoleLines = [];
    renderConsole();
  });
  const copyButton = byId('btn-copy-console');
  copyButton.addEventListener('click', async () => {
    const selected = window.getSelection()?.toString();
    try {
      await navigator.clipboard.writeText(selected || consoleLines.join('\n'));
      const original = copyButton.textContent;
      copyButton.textContent = '✅ Скопировано';
      setTimeout(() => { copyButton.textContent = original; }, 2000);
    } catch (error) {
      appendConsole(`[Консоль] Не удалось скопировать: ${error}`);
    }
  });
  byId('btn-save-debug').addEventListener('click', async () => {
    const customPath = await tauriDialog?.save?.({
      title: 'Сохранить журнал лаунчера',
      defaultPath: 'fenixcraft-debug.log',
      filters: [{ name: 'Log Files', extensions: ['log', 'txt'] }],
    });
    try {
      const path = await invoke('save_debug_log', { consoleText: consoleLines.join('\n'), customPath: customPath ?? null });
      appendConsole(`[Консоль] Журнал сохранён: ${path}`);
    } catch (error) {
      appendConsole(`[Консоль] Не удалось сохранить журнал: ${error}`);
    }
  });
  byId('btn-verify-assets').addEventListener('click', () => {
    invoke('verify_assets_command').catch((error) => appendConsole(`[Проверка] Ошибка: ${error}`));
  });
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
  pill.className = `flex items-center space-x-1.5 text-[11px] font-medium no-drag ${tone.text}`;
  pill.title = label.hint;
  byId('network-dot').className = `w-1.5 h-1.5 rounded-full ${tone.dot}`;
  byId('network-text').textContent = label.text;
  if (state.internet && !wasInternetAvailable) {
    loadNews();
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
  byId('twitch-label').classList.toggle('twitch-live-active', live);
  byId('twitch-label').textContent = live ? 'Twitch • LIVE' : 'Twitch';
  byId('twitch-live-dot').classList.toggle('hidden', !live);
}

async function checkForUpdates() {
  let update;
  try {
    update = await invoke('check_launcher_update');
  } catch {
    return;
  }
  if (!update?.updateAvailable) return;

  const modal = byId('update-modal');
  const headerButton = byId('btn-update-launcher');
  byId('update-version-label').textContent = `Доступна версия v${update.latestVersion}`;
  byId('update-changelog').textContent = update.changelog || 'Улучшена стабильность и исправлены ошибки.';
  headerButton.textContent = `⬇ Обновить до v${update.latestVersion}`;
  headerButton.classList.remove('hidden');
  headerButton.addEventListener('click', () => modal.classList.remove('hidden'));
  byId('btn-update-later').addEventListener('click', () => modal.classList.add('hidden'));
  const installButton = byId('btn-update-now');
  installButton.addEventListener('click', async () => {
    installButton.disabled = true;
    installButton.textContent = 'Загрузка...';
    try {
      await invoke('install_launcher_update', { downloadUrl: update.downloadUrl });
    } catch (error) {
      modal.classList.add('hidden');
      showError({ title: 'Обновление не установлено', message: String(error), hint: 'Попробуйте позже или скачайте лаунчер со страницы релизов.' });
    } finally {
      installButton.disabled = false;
      installButton.textContent = 'Обновить сейчас';
    }
  });
  modal.classList.remove('hidden');
}

// --- Общие обработчики ---

function bindWindowControls() {
  const commands = { 'btn-win-min': 'window_minimize', 'btn-win-max': 'window_maximize', 'btn-win-close': 'window_close' };
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
    const nickname = encodeURIComponent(currentConfig.username || 'Player');
    openUrl(`${SHOP_URL}?nickname=${nickname}&item=${encodeURIComponent(buy.dataset.buy)}`);
    return;
  }
  const server = target.closest('[data-server-ip]');
  if (server) {
    persistConfig({ selectedServer: server.dataset.serverIp }).then(refreshServers);
    return;
  }
  const tabLink = target.closest('[data-open-tab]');
  if (tabLink) {
    switchTab(tabLink.dataset.openTab);
    return;
  }
  const profile = target.closest('[data-select-profile]');
  if (profile) {
    const select = byId('select-version');
    select.value = profile.dataset.selectProfile;
    select.dispatchEvent(new Event('input'));
  }
}

function bindDelegatedActions() {
  document.addEventListener('click', (event) => activateElement(event.target));
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') hideError();
    const isRoleButton = event.target.getAttribute?.('role') === 'button';
    if (isRoleButton && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      activateElement(event.target);
    }
  });
  // Если картинка новости не загрузилась, показываем локальный баннер.
  document.addEventListener('error', (event) => {
    const image = event.target;
    if (image.classList?.contains('news-image') && !image.src.endsWith(LOCAL_NEWS_IMAGE)) {
      image.src = LOCAL_NEWS_IMAGE;
    }
  }, true);

  document.querySelectorAll('.nav-btn').forEach((button) => {
    button.addEventListener('click', () => switchTab(button.dataset.tab));
  });
  byId('btn-play').addEventListener('click', startGame);
  byId('btn-twitch-nav').addEventListener('click', () => openUrl(TWITCH_URL));
  byId('btn-refresh-news').addEventListener('click', loadNews);
  byId('btn-ping-servers').addEventListener('click', refreshServers);
  byId('btn-save-server').addEventListener('click', saveCustomServer);
  byId('btn-reset-servers').addEventListener('click', async () => {
    await persistConfig({ customServers: SERVER_DEFAULTS });
    await refreshServers();
  });
  byId('btn-error-close').addEventListener('click', hideError);
  byId('btn-error-console').addEventListener('click', () => {
    hideError();
    switchTab('console');
  });
}

async function showLauncherVersion() {
  const version = await invoke('get_launcher_version');
  byId('app-version-badge').textContent = `v${version}`;
}

async function start() {
  bindWindowControls();
  bindDelegatedActions();
  bindConsole();
  bindGameEvents();

  currentConfig = await invoke('get_config');
  bindSettings();
  bindProfile();
  renderProfile();
  renderShop();
  renderServers({});

  // Интерфейс уже готов: сетевые данные догружаются в фоне и не задерживают запуск.
  showLauncherVersion();
  loadNews();
  refreshServers();
  refreshConnectivity();
  updateTwitchStatus();
  checkForUpdates();
  setInterval(refreshServers, REFRESH_INTERVAL_MS);
  setInterval(updateTwitchStatus, REFRESH_INTERVAL_MS);
  setInterval(refreshConnectivity, CONNECTIVITY_INTERVAL_MS);
}

start().catch((error) => showError({ title: 'Лаунчер запустился с ошибкой', message: String(error), hint: 'Перезапустите лаунчер.' }));
