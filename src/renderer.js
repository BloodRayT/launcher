import {
  connectivityLabel,
  escapeHtml,
  formatReleaseDate,
  isValidUsername,
  pickLocalImage,
  renderMarkdown,
  safeImageUrl,
  safeLinkUrl,
  serverStatusLabel,
} from './utils.js';

const tauriCore = window.__TAURI__?.core;
const tauriEvent = window.__TAURI__?.event;
const tauriDialog = window.__TAURI__?.dialog;

const LOCAL_NEWS_IMAGES = ['baner.webp', 'background.jpg'];
const NEWS_CACHE_KEY = 'fenix.news.v2';
const RELEASES_CACHE_KEY = 'fenix.releases.v1';
const RELEASES_FETCHED_KEY = 'fenix.releases.fetchedAt';
const RELEASES_REFRESH_MS = 30 * 60 * 1000;
const NEWS_CARDS_ON_HOME = 2;
const TWITCH_CHANNEL = 'bloodrayt';
const SHOP_URL = 'https://fenixcraft.trademc.org/';
const TWITCH_URL = 'https://www.twitch.tv/bloodrayt';
const REFRESH_INTERVAL_MS = 30000;
const CONNECTIVITY_INTERVAL_MS = 60000;
const SAVE_DEBOUNCE_MS = 300;
const MAX_CONSOLE_LINES = 5000;
const MAX_AVATAR_BYTES = 2 * 1024 * 1024;
const AVATAR_SIZE = 128;
const VALID_PROFILES = ['26.3-vanilla', '26.3-iris', '1.21.1-neoforge', '1.21.1-iris-neoforge'];
const TABS = ['home', 'shop', 'servers', 'settings', 'faq', 'console'];

/** Вне приложения (проверка верстки в браузере) вызовы заменяются заглушками. */
const previewResponses = {
  get_config: () => ({
    username: 'Preview_Player', accountType: 'offline', ram: 6144, width: 1280, height: 720,
    javaPath: '', jvmArgs: '-XX:+UseG1GC', showConsole: false, debugMode: false,
    selectedVersion: '26.3-vanilla', selectedServer: 'play.fenixcraft.ru',
    customServers: [], avatarData: '', msUsername: '', msUuid: '',
  }),
  get_launcher_version: () => 'dev',
  get_connectivity: () => ({ internet: true, backend: true }),
  get_servers_status: () => ({}),
  get_vk_news: () => ({ success: false }),
  check_twitch_live: () => ({ live: false }),
  get_launcher_releases: () => [],
  check_launcher_update: () => ({ updateAvailable: false }),
  ms_login_available: () => false,
  save_launcher_config: (args) => args.newConfig,
};

function invoke(command, args) {
  if (tauriCore) return tauriCore.invoke(command, args);
  return Promise.resolve(previewResponses[command]?.(args));
}

const SERVER_DEFAULTS = [
  { id: 'fenix-main', name: 'FenixCraft', desc: 'Vanilla • 26.3', ip: 'play.fenixcraft.ru', port: 25565, icon: '🦅', maxDefault: 1000 },
  { id: 'fenix-mods', name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', port: 25565, icon: '⚙️', maxDefault: 500 },
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
  { tag: 'VK.RU/FENIXCRAFT', title: 'Сообщество FenixCraft ВКонтакте', text: 'Новости появятся здесь, как только лаунчер сможет связаться с ВКонтакте. Все записи и конкурсы есть на стене сообщества.', date: 'Официальная группа', image: '', images: [], url: 'https://vk.ru/fenixcraft' },
  { tag: 'Магазин', tone: 'amber', title: 'Привилегии VIP, Creative, Moder, Admin и Money', text: 'Поддержите развитие сервера и получите уникальные возможности в разделе «Магазин».', date: 'FenixCraft Shop', image: '', images: [], url: SHOP_URL },
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
  const buyer = escapeHtml(displayName() || 'Игрока');
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

let newsPosts = [];
let releaseNotes = [];

function imageOf(post) {
  return safeImageUrl(post.image, pickLocalImage(post.title, LOCAL_NEWS_IMAGES));
}

function newsCard({ attributes, image, tag, tagClass, title, text, date }) {
  return `
    <div role="button" tabindex="0" ${attributes}
         class="glass-card rounded-xl overflow-hidden flex flex-col justify-between hover:border-emerald-400/50 transition cursor-pointer">
      <div class="h-28 relative bg-slate-900">
        <img src="${escapeHtml(image)}" alt="" loading="lazy" class="news-image w-full h-full object-cover" />
        <span class="absolute bottom-2 left-2 text-[10px] font-bold px-2.5 py-0.5 rounded-full ${tagClass}">${escapeHtml(tag)}</span>
      </div>
      <div class="p-3.5 flex-1 flex flex-col justify-between space-y-2">
        <div>
          <h4 class="text-sm font-bold text-white line-clamp-1">${escapeHtml(title)}</h4>
          <p class="text-xs text-slate-300 mt-1 line-clamp-2">${escapeHtml(text)}</p>
        </div>
        <div class="text-[11px] text-emerald-300 pt-1 flex justify-between items-center">
          <span>${escapeHtml(date)}</span>
          <span>Подробнее</span>
        </div>
      </div>
    </div>`;
}

function renderNewsGrid() {
  const posts = newsPosts.length > 0 ? newsPosts : FALLBACK_NEWS;
  const latest = releaseNotes[0];
  const cards = posts.slice(0, NEWS_CARDS_ON_HOME).map((post, index) => newsCard({
    attributes: `data-open-news="${index}"`,
    image: imageOf(post),
    tag: post.tag || 'VK.RU/FENIXCRAFT',
    tagClass: newsTagClass(post),
    title: post.title || 'Новость FenixCraft',
    text: post.text,
    date: post.date,
  }));
  cards.push(newsCard({
    attributes: 'data-open-updates="true"',
    image: LOCAL_NEWS_IMAGES[0],
    tag: 'Лаунчер',
    tagClass: 'bg-cyan-500 text-slate-950',
    title: latest ? `Обновление ${latest.version}` : 'Обновления лаунчера',
    text: latest ? latest.body.replace(/[#*`]/g, '').trim() || latest.name : 'История версий появится, когда лаунчер свяжется с GitHub.',
    date: latest ? formatReleaseDate(latest.date) : 'GitHub',
  }));
  byId('news-grid').innerHTML = cards.join('');
}

function readCache(key) {
  try {
    const cached = JSON.parse(localStorage.getItem(key));
    return Array.isArray(cached) ? cached : [];
  } catch {
    return [];
  }
}

function writeCache(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Кеш необязателен: без него покажутся карточки по умолчанию.
  }
}

function releasesAreFresh() {
  if (readCache(RELEASES_CACHE_KEY).length === 0) return false;
  try {
    return Date.now() - Number(localStorage.getItem(RELEASES_FETCHED_KEY)) < RELEASES_REFRESH_MS;
  } catch {
    return false;
  }
}

function writeReleasesFetchedAt() {
  try {
    localStorage.setItem(RELEASES_FETCHED_KEY, String(Date.now()));
  } catch {
    // Без отметки времени история просто запросится ещё раз.
  }
}

async function loadNews() {
  newsPosts = readCache(NEWS_CACHE_KEY);
  releaseNotes = readCache(RELEASES_CACHE_KEY);
  renderNewsGrid();
  const [news, releases] = await Promise.allSettled([
    invoke('get_vk_news'),
    // У GitHub строгий лимит запросов без авторизации, поэтому история обновляется редко.
    releasesAreFresh() ? Promise.resolve(null) : invoke('get_launcher_releases'),
  ]);
  if (news.status === 'fulfilled' && news.value?.success && news.value.posts?.length > 0) {
    newsPosts = news.value.posts;
    writeCache(NEWS_CACHE_KEY, newsPosts);
  }
  if (releases.status === 'fulfilled' && releases.value?.length > 0) {
    releaseNotes = releases.value;
    writeCache(RELEASES_CACHE_KEY, releaseNotes);
    writeReleasesFetchedAt();
  }
  renderNewsGrid();
}

// --- Стеклянное окно ---

let glassModalOpener = null;

/** Пока открыто окно, основной интерфейс недоступен и для клавиатуры. */
function setBackgroundInert(inert) {
  document.querySelectorAll('body > header, body > .flex-1').forEach((element) => {
    element.inert = inert;
  });
}

function openGlassModal({ title, html, wide = false }) {
  glassModalOpener = document.activeElement;
  byId('glass-modal-title').textContent = title;
  byId('glass-modal-body').innerHTML = html;
  byId('glass-modal-body').scrollTop = 0;
  byId('glass-modal-panel').classList.toggle('glass-panel-wide', wide);
  byId('glass-modal').classList.remove('hidden');
  setBackgroundInert(true);
  byId('btn-glass-close').focus();
}

function closeGlassModal() {
  if (byId('glass-modal').classList.contains('hidden')) return;
  byId('glass-modal').classList.add('hidden');
  setBackgroundInert(false);
  // Удаление содержимого останавливает плеер и звук трансляции.
  byId('glass-modal-body').innerHTML = '';
  glassModalOpener?.focus?.();
}

function newsArticle(post, index) {
  const images = (post.images?.length ? post.images : [post.image]).map((url) => safeImageUrl(url, '')).filter(Boolean);
  const shown = images.length > 0 ? images : [imageOf(post)];
  const url = safeLinkUrl(post.url);
  return `
    <article id="news-post-${index}" class="glass-article">
      <div class="glass-article-images ${shown.length > 1 ? 'is-gallery' : ''}">
        ${shown.map((src) => `<img src="${escapeHtml(src)}" alt="" loading="lazy" class="news-image" />`).join('')}
      </div>
      <div class="glass-article-meta">
        <span>${escapeHtml(post.date)}</span>
        ${url ? `<button data-external="${escapeHtml(url)}" class="glass-link">Открыть в браузере</button>` : ''}
      </div>
      <h4 class="glass-article-title">${escapeHtml(post.title)}</h4>
      <p class="glass-article-text selectable-console">${escapeHtml(post.text)}</p>
    </article>`;
}

function openNewsModal(index) {
  const posts = newsPosts.length > 0 ? newsPosts : FALLBACK_NEWS;
  openGlassModal({ title: 'Новости FenixCraft', html: posts.map(newsArticle).join('') });
  byId(`news-post-${index}`)?.scrollIntoView({ block: 'start' });
}

function openUpdatesModal() {
  const body = releaseNotes.length > 0
    ? releaseNotes.map((release) => `
      <article class="glass-article">
        <div class="glass-article-meta">
          <span>${escapeHtml(formatReleaseDate(release.date))}</span>
          <button data-external="${escapeHtml(safeLinkUrl(release.url) || 'https://github.com/BloodRayT/launcher/releases')}" class="glass-link">Страница релиза</button>
        </div>
        <h4 class="glass-article-title">${escapeHtml(release.name)}</h4>
        <div class="glass-markdown selectable-console">${renderMarkdown(release.body) || '<p>Описание для этой версии не добавлено.</p>'}</div>
      </article>`).join('')
    : '<p class="glass-article-text">История обновлений пока недоступна. Проверьте подключение к интернету.</p>';
  openGlassModal({ title: 'Обновления лаунчера', html: body });
}

function openTwitchModal() {
  // Twitch требует указать адрес встраивающей страницы.
  const parent = encodeURIComponent(location.hostname);
  openGlassModal({
    title: 'Трансляция BloodRayT',
    wide: true,
    html: `
      <div class="twitch-layout">
        <iframe class="twitch-player" title="Плеер Twitch" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen
                src="https://player.twitch.tv/?channel=${TWITCH_CHANNEL}&parent=${parent}&autoplay=true"></iframe>
        <iframe class="twitch-chat" title="Чат Twitch"
                src="https://www.twitch.tv/embed/${TWITCH_CHANNEL}/chat?parent=${parent}&darkpopout"></iframe>
      </div>
      <div class="glass-article-meta twitch-footer">
        <span>Плеер разворачивается на весь экран кнопкой в его правом нижнем углу</span>
        <button data-external="${TWITCH_URL}" class="glass-link">Открыть в браузере</button>
      </div>`,
  });
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
  fields.version.value = VALID_PROFILES.includes(currentConfig.selectedVersion) ? currentConfig.selectedVersion : VALID_PROFILES[0];

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

let accountTab = 'offline';
let microsoftAvailable = false;
let nickTimer = null;

const NICK_RULES_MESSAGE = 'Ник: 3–16 латинских букв, цифр или _';

function isLicensed() {
  return currentConfig.accountType === 'microsoft';
}

function displayName() {
  return isLicensed() ? currentConfig.msUsername : currentConfig.username;
}

function renderProfile() {
  document.querySelectorAll('[data-account-tab]').forEach((tab) => {
    tab.setAttribute('aria-pressed', String(tab.dataset.accountTab === accountTab));
  });
  const signedIn = Boolean(currentConfig.msUuid);
  byId('sidebar-offline-box').classList.toggle('hidden', accountTab !== 'offline');
  byId('sidebar-license-box').classList.toggle('hidden', accountTab !== 'microsoft');
  byId('btn-ms-login').classList.toggle('hidden', signedIn);
  byId('btn-ms-logout').classList.toggle('hidden', !signedIn);
  byId('btn-ms-login').disabled = !microsoftAvailable;
  byId('btn-ms-login').textContent = microsoftAvailable ? 'Войти через Microsoft' : 'Вход пока не настроен';
  byId('sidebar-account-type').textContent = isLicensed() ? 'Лицензия' : 'Пиратка';
  byId('sidebar-username').textContent = displayName() || 'Новый игрок';
  const nickInput = byId('sidebar-nick-input');
  if (document.activeElement !== nickInput) nickInput.value = currentConfig.username || '';
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

  byId('btn-ms-login').addEventListener('click', startMicrosoftLogin);
  byId('btn-ms-logout').addEventListener('click', async () => {
    try {
      currentConfig = await invoke('ms_logout');
      accountTab = 'offline';
      refreshProfileViews();
    } catch (error) {
      showError(error);
    }
  });
  byId('btn-ms-cancel').addEventListener('click', () => {
    invoke('ms_login_cancel');
    byId('ms-login-modal').classList.add('hidden');
  });
  byId('btn-ms-copy').addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(byId('ms-login-code').textContent);
      flashAuthMessage('Код скопирован');
    } catch {
      flashAuthMessage('Не удалось скопировать, выделите код вручную');
    }
  });
  byId('btn-ms-open').addEventListener('click', () => openUrl(microsoftLoginUrl));
  tauriEvent?.listen('ms:login-done', ({ payload }) => {
    currentConfig = payload;
    accountTab = 'microsoft';
    byId('ms-login-modal').classList.add('hidden');
    refreshProfileViews();
    flashAuthMessage(`Вход выполнен: ${payload.msUsername}`);
  });
  tauriEvent?.listen('ms:login-failed', ({ payload }) => {
    byId('ms-login-modal').classList.add('hidden');
    showError(payload);
  });
}

function refreshProfileViews() {
  renderProfile();
  renderShop();
}

async function selectAccountTab(tab) {
  accountTab = tab;
  byId('sidebar-auth-msg').textContent = '';
  if (tab === 'offline' && isLicensed()) {
    await persistConfig({ accountType: 'offline' });
  } else if (tab === 'microsoft' && currentConfig.msUuid && !isLicensed()) {
    await persistConfig({ accountType: 'microsoft' });
  }
  refreshProfileViews();
}

let microsoftLoginUrl = '';

async function startMicrosoftLogin() {
  const button = byId('btn-ms-login');
  button.disabled = true;
  try {
    const login = await invoke('ms_login_start');
    microsoftLoginUrl = login.verificationUri;
    byId('ms-login-code').textContent = login.userCode;
    byId('ms-login-modal').classList.remove('hidden');
    byId('btn-ms-open').focus();
  } catch (error) {
    showError(error);
  } finally {
    button.disabled = false;
  }
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
  byId('btn-twitch-nav').classList.toggle('twitch-live-active', live);
  byId('twitch-live-badge').classList.toggle('hidden', !live);
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
    const nickname = encodeURIComponent(displayName() || 'Player');
    openUrl(`${SHOP_URL}?nickname=${nickname}&item=${encodeURIComponent(buy.dataset.buy)}`);
    return;
  }
  const server = target.closest('[data-server-ip]');
  if (server) {
    persistConfig({ selectedServer: server.dataset.serverIp }).then(refreshServers);
    return;
  }
  const newsCardElement = target.closest('[data-open-news]');
  if (newsCardElement) {
    openNewsModal(Number(newsCardElement.dataset.openNews));
    return;
  }
  if (target.closest('[data-open-updates]')) {
    openUpdatesModal();
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
    if (event.key === 'Escape') {
      // Закрывается только верхнее окно.
      if (byId('error-modal').classList.contains('hidden')) closeGlassModal();
      else hideError();
    }
    const isRoleButton = event.target.getAttribute?.('role') === 'button';
    if (isRoleButton && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      activateElement(event.target);
    }
  });
  // Если картинка новости не загрузилась, показываем локальный баннер.
  document.addEventListener('error', (event) => {
    const image = event.target;
    if (image.classList?.contains('news-image') && !image.src.endsWith(LOCAL_NEWS_IMAGES[0])) {
      image.src = LOCAL_NEWS_IMAGES[0];
    }
  }, true);

  document.querySelectorAll('.nav-btn').forEach((button) => {
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
  accountTab = isLicensed() ? 'microsoft' : 'offline';
  microsoftAvailable = Boolean(await invoke('ms_login_available'));
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
