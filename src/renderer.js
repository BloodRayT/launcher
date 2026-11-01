// FenixCraft Launcher Renderer v1.0.4
const tauriCore = window.__TAURI__ ? (window.__TAURI__.core || window.__TAURI__.tauri) : null;

window.fenixAPI = {
  getConfig: () => tauriCore ? tauriCore.invoke('get_config') : Promise.resolve({}),
  saveConfig: (cfg) => tauriCore ? tauriCore.invoke('save_config', { newConfig: cfg }) : Promise.resolve(),
  launchGame: (profile, nick, ram) => tauriCore ? tauriCore.invoke('launch_game', { profile, nick, ram }) : Promise.resolve(),
  mojangAuth: (username, password) => tauriCore ? tauriCore.invoke('mojang_authenticate', { username, password }) : Promise.resolve({ success: false, error: 'Tauri недоступен' }),
  openFolder: (p) => tauriCore ? tauriCore.invoke('open_game_folder', { path: p }) : Promise.resolve(),
};

let currentConfig = {
  username: 'BloodRayT',
  accountType: 'offline',
  ram: 6144
};

// Функция переключения вкладок (строго изолированная)
window.switchTab = function(tabName) {
  // 1. Скрываем все экраны
  document.querySelectorAll('.tab-content').forEach(tab => {
    tab.classList.remove('active');
  });

  // 2. Открываем целевую
  const target = document.getElementById(`tab-${tabName}`);
  if (target) {
    target.classList.add('active');
  }

  // 3. Переключаем стиль кнопок в меню
  document.querySelectorAll('.nav-btn').forEach(btn => {
    if (btn.getAttribute('data-tab') === tabName) {
      btn.className = 'nav-btn w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-bold transition bg-emerald-500/20 text-emerald-400 border border-emerald-500/30';
    } else {
      btn.className = 'nav-btn w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-bold transition text-slate-400 hover:bg-white/5 hover:text-white';
    }
  });

  // При открытии вкладки Twitch обновляем плеер
  if (tabName === 'twitch') {
    initTwitchPlayer();
  }
};

// Мгновенное обновление никнейма в DOM
function applyNickToUI(name, typeBadge) {
  const finalName = (name && name.trim() !== '') ? name.trim() : 'Новый игрок';
  const userEl = document.getElementById('sidebar-username');
  if (userEl) userEl.textContent = finalName;

  const accEl = document.getElementById('sidebar-account-type');
  if (accEl && typeBadge) accEl.textContent = typeBadge;

  const inputEl = document.getElementById('sidebar-nick-input');
  if (inputEl && inputEl.value !== finalName) inputEl.value = finalName;
}

// Умный плеер Twitch (Live Stream -> Последний VOD)
let isStreamLoaded = false;
async function initTwitchPlayer() {
  const box = document.getElementById('twitch-player-box');
  const badge = document.getElementById('twitch-mode-badge');
  const subStatus = document.getElementById('twitch-sub-status');
  if (!box) return;

  // Проверяем онлайн через прямой стрим
  box.innerHTML = `
    <iframe
      src="https://player.twitch.tv/?channel=bloodrayt&parent=localhost&autoplay=true&muted=false"
      class="w-full h-full border-0 rounded-2xl"
      allowfullscreen>
    </iframe>
  `;

  if (badge) badge.textContent = 'Прямой эфир / VOD';
  if (subStatus) subStatus.textContent = 'Плеер канала BloodRayT (Live & Записи стримов)';
}

// Управление профилем игрока
function setupProfile() {
  const saveBtn = document.getElementById('btn-sidebar-save-nick');
  const nickInput = document.getElementById('sidebar-nick-input');
  const accSelect = document.getElementById('sidebar-acc-type');
  const offlineBox = document.getElementById('sidebar-offline-box');
  const licenseBox = document.getElementById('sidebar-license-box');
  const authMsg = document.getElementById('sidebar-auth-msg');

  if (accSelect) {
    accSelect.addEventListener('change', () => {
      if (accSelect.value === 'mojang') {
        offlineBox?.classList.add('hidden');
        licenseBox?.classList.remove('hidden');
      } else {
        offlineBox?.classList.remove('hidden');
        licenseBox?.classList.add('hidden');
      }
    });
  }

  if (saveBtn && nickInput) {
    saveBtn.addEventListener('click', async (e) => {
      e.preventDefault();
      const newNick = nickInput.value.trim();
      if (!newNick) {
        if (authMsg) authMsg.textContent = 'Введите никнейм!';
        return;
      }

      // МГНОВЕННО меняем текст в шапке
      applyNickToUI(newNick, 'Пиратка');

      currentConfig.username = newNick;
      currentConfig.accountType = 'offline';
      await window.fenixAPI.saveConfig(currentConfig);

      saveBtn.textContent = '✅ Сохранено!';
      if (authMsg) authMsg.textContent = `Ник сохранен: ${newNick}`;
      setTimeout(() => {
        saveBtn.textContent = 'Сохранить ник';
        if (authMsg) authMsg.textContent = '';
      }, 2000);
    });
  }

  const loginLicBtn = document.getElementById('btn-sidebar-login-license');
  const licLoginInput = document.getElementById('sidebar-license-login');
  const licPassInput = document.getElementById('sidebar-license-pass');
  if (loginLicBtn) {
    loginLicBtn.addEventListener('click', async () => {
      const login = licLoginInput?.value.trim();
      const pass = licPassInput?.value.trim();
      if (!login || !pass) return;

      loginLicBtn.textContent = 'Вход...';
      const res = await window.fenixAPI.mojangAuth(login, pass);
      if (res && res.success) {
        applyNickToUI(res.username, 'Лицензия');
        currentConfig.username = res.username;
        currentConfig.accountType = 'mojang';
        await window.fenixAPI.saveConfig(currentConfig);
        loginLicBtn.textContent = '✅ Успешно!';
      } else {
        loginLicBtn.textContent = 'Ошибка';
      }
      setTimeout(() => { loginLicBtn.textContent = 'Войти (Mojang)'; }, 2000);
    });
  }
}

// Загрузка товаров магазина
function renderShop() {
  const container = document.getElementById('shop-cards-grid');
  if (!container) return;
  const items = [
    { title: 'Привилегия VIP', price: '149 ₽', desc: 'Цветной ник, полет в лобби, доступ к /kit vip' },
    { title: 'Привилегия Premium', price: '349 ₽', desc: 'Все возможности VIP + /feed, /workbench, 5 точек дома' },
    { title: 'Привилегия Creative', price: '599 ₽', desc: 'Креативный режим, доступ к миру построек, /fly везде' },
    { title: 'Привилегия Admin', price: '1299 ₽', desc: 'Максимальный набор прав, префикс Администратора, бан/мут' },
    { title: '1000 FenixCoins', price: '99 ₽', desc: 'Внутриигровая валюта для покупки кейсов и косметики' },
    { title: '5000 FenixCoins', price: '399 ₽', desc: 'Выгодный пакет монет с бонусом +15% к начислению' },
  ];
  container.innerHTML = items.map(it => `
    <div class="p-4 rounded-xl glass-card border border-white/5 space-y-2 flex flex-col justify-between">
      <div>
        <div class="flex items-center justify-between">
          <span class="font-bold text-white text-sm">${it.title}</span>
          <span class="text-xs font-black text-emerald-400 px-2 py-0.5 rounded bg-emerald-500/10">${it.price}</span>
        </div>
        <p class="text-[11px] text-slate-400 mt-1">${it.desc}</p>
      </div>
      <a href="https://fenixcraft.ru/shop" target="_blank" class="mt-3 block text-center py-1.5 bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-bold rounded-lg text-xs transition">Купить</a>
    </div>
  `).join('');
}

// Рендер серверов в боковой панели и на вкладке
function renderServers() {
  const servers = [
    { name: 'FenixCraft', desc: 'Выживание • Vanilla/Mods', ip: 'play.fenixcraft.ru', online: '0/1000' },
    { name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', online: '0/500' },
    { name: 'FenixRPG', desc: 'Fabric + Iris • 1.21.11', ip: 'rpg.fenixcraft.ru', online: '0/500' },
    { name: 'FenixArena', desc: 'Анархия • 1.21.11', ip: 'pvp.fenixcraft.ru', online: '0/300' },
  ];

  const quick = document.getElementById('sidebar-servers-quick');
  if (quick) {
    quick.innerHTML = servers.map(s => `
      <div class="p-2.5 rounded-xl glass-card flex items-center justify-between text-xs">
        <div>
          <div class="font-bold text-white">${s.name}</div>
          <div class="text-[10px] text-slate-400">${s.desc}</div>
        </div>
        <div class="text-[10px] text-emerald-400 font-bold flex items-center space-x-1">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
          <span>${s.online}</span>
        </div>
      </div>
    `).join('');
  }

  const full = document.getElementById('servers-list-container');
  if (full) {
    full.innerHTML = servers.map(s => `
      <div class="p-4 rounded-xl glass-card flex items-center justify-between">
        <div>
          <h3 class="text-sm font-bold text-white">${s.name}</h3>
          <p class="text-xs text-slate-400">${s.desc} • IP: <span class="text-emerald-400 font-mono">${s.ip}</span></p>
        </div>
        <button class="px-4 py-1.5 bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-bold rounded-xl transition" onclick="document.getElementById('btn-play').click()">Подключиться</button>
      </div>
    `).join('');
  }
}

// Рендер новостей ВК
function renderNews() {
  const grid = document.getElementById('vk-news-grid');
  if (!grid) return;
  const news = [
    { title: 'Сообщество FenixCraft ВКонтакте', tag: 'VK.RU/FENIXCRAFT', text: 'Следите за обновлениями, конкурсами и техническими работами.', link: 'https://vk.com/fenixcraft' },
    { title: 'Привилегии VIP, Creative, Admin', tag: 'Магазин', text: 'Поддержите развитие проекта и получите уникальные возможности.', link: 'https://fenixcraft.ru/shop' },
    { title: 'Автозагрузка Java 21 Adoptium', tag: 'Релиз v1.0.4', text: 'Лаунчер автоматически скачивает чистую Java для максимального FPS.', link: 'https://fenixcraft.ru' }
  ];
  grid.innerHTML = news.map(n => `
    <div class="p-4 rounded-xl glass-card border border-white/5 space-y-2 flex flex-col justify-between">
      <div>
        <span class="text-[10px] font-bold text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded">${n.tag}</span>
        <h4 class="font-bold text-white text-xs mt-2">${n.title}</h4>
        <p class="text-[11px] text-slate-400 mt-1">${n.text}</p>
      </div>
      <a href="${n.link}" target="_blank" class="text-xs text-emerald-400 hover:underline pt-2">Читать далее →</a>
    </div>
  `).join('');
}

// Инициализация
document.addEventListener('DOMContentLoaded', async () => {
  // Навигация
  document.querySelectorAll('#sidebar-nav button').forEach(btn => {
    btn.addEventListener('click', () => {
      const tab = btn.getAttribute('data-tab');
      if (tab) switchTab(tab);
    });
  });

  setupProfile();
  renderShop();
  renderServers();
  renderNews();

  const reloadStreamBtn = document.getElementById('btn-reload-stream');
  if (reloadStreamBtn) {
    reloadStreamBtn.addEventListener('click', () => initTwitchPlayer());
  }

  // Загружаем конфиг
  try {
    const cfg = await window.fenixAPI.getConfig();
    if (cfg && cfg.username) {
      currentConfig = cfg;
      applyNickToUI(cfg.username, cfg.accountType === 'mojang' ? 'Лицензия' : 'Пиратка');
    }
  } catch(e) {}

  // Кнопка Играть
  const btnPlay = document.getElementById('btn-play');
  const selVer = document.getElementById('select-version');
  if (btnPlay) {
    btnPlay.addEventListener('click', async () => {
      btnPlay.disabled = true;
      btnPlay.innerHTML = '<span>⏳ Запуск...</span>';
      await window.fenixAPI.launchGame(selVer?.value || 'latest-vanilla', currentConfig.username, currentConfig.ram);
      setTimeout(() => {
        btnPlay.disabled = false;
        btnPlay.innerHTML = '<span class="text-base">▶</span><span class="text-sm tracking-wide">Играть</span>';
      }, 4000);
    });
  }

  const btnFolder = document.getElementById('btn-open-folder');
  if (btnFolder) {
    btnFolder.addEventListener('click', () => window.fenixAPI.openFolder(''));
  }
});
