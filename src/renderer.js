// Инициализация моста Tauri API
const tauriCore = window.__TAURI__ ? (window.__TAURI__.core || window.__TAURI__.tauri) : null;

window.fenixAPI = {
  getConfig: () => tauriCore ? tauriCore.invoke('get_config') : Promise.resolve({}),
  saveConfig: (cfg) => tauriCore ? tauriCore.invoke('save_config', { newConfig: cfg }) : Promise.resolve(),
  launchGame: (profile, nick, ram) => tauriCore ? tauriCore.invoke('launch_game', { profile, nick, ram }) : Promise.resolve(),
  mojangAuth: (username, password) => tauriCore ? tauriCore.invoke('mojang_authenticate', { username, password }) : Promise.resolve({ success: false, error: 'Tauri недоступен' }),
  checkLauncherUpdate: () => tauriCore ? tauriCore.invoke('check_launcher_update') : Promise.resolve({ updateAvailable: false }),
  openFolder: (p) => tauriCore ? tauriCore.invoke('open_game_folder', { path: p }) : Promise.resolve(),
};

let currentConfig = {
  username: 'BloodRayT',
  accountType: 'offline',
  ram: 6144,
  avatarData: ''
};

// Функция надежного обновления имени в DOM
function applyNickToUI(name, typeBadge) {
  const finalName = (name && name.trim() !== '') ? name.trim() : 'Новый игрок';
  
  // 1. Ищем и обновляем текстовый узел
  const userEl = document.getElementById('sidebar-username');
  if (userEl) userEl.textContent = finalName;

  const accEl = document.getElementById('sidebar-account-type');
  if (accEl && typeBadge) accEl.textContent = typeBadge;

  // 2. Синхронизируем инпут
  const inputEl = document.getElementById('sidebar-nick-input');
  if (inputEl && inputEl.value !== finalName) inputEl.value = finalName;
}

// Переключение табов
function setupTabs() {
  const navButtons = document.querySelectorAll('#sidebar-nav button');
  const mainScreens = document.querySelectorAll('main');

  navButtons.forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      const tabName = btn.getAttribute('data-tab');

      // Скрываем все экраны
      mainScreens.forEach(s => s.classList.add('hidden'));

      // Показываем целевой
      const targetScreen = document.getElementById(`tab-${tabName}`);
      if (targetScreen) {
        targetScreen.classList.remove('hidden');
        if (tabName === 'twitch') targetScreen.style.display = 'flex';
      }

      // Переключаем стили активной кнопки
      navButtons.forEach(b => {
        b.className = 'w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-bold transition text-slate-400 hover:bg-white/5 hover:text-white';
      });
      btn.className = 'w-full flex items-center space-x-3 px-3 py-2.5 rounded-xl text-xs font-bold transition bg-emerald-500/20 text-emerald-400 border border-emerald-500/30';
    });
  });
}

// Управление профилем игрока
function setupProfile() {
  const saveBtn = document.getElementById('btn-sidebar-save-nick');
  const nickInput = document.getElementById('sidebar-nick-input');
  const accSelect = document.getElementById('sidebar-acc-type');
  const offlineBox = document.getElementById('sidebar-offline-box');
  const licenseBox = document.getElementById('sidebar-license-box');
  const authMsg = document.getElementById('sidebar-auth-msg');

  // Переключение типа аккаунта
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

  // Мгновенное сохранение ника
  if (saveBtn && nickInput) {
    saveBtn.addEventListener('click', async (e) => {
      e.preventDefault();
      const newNick = nickInput.value.trim();
      if (!newNick) {
        if (authMsg) authMsg.textContent = 'Введите никнейм!';
        return;
      }

      // 1. Мгновенно перерисовываем текст
      applyNickToUI(newNick, 'Пиратка');

      // 2. Обновляем память и сохраняем
      currentConfig.username = newNick;
      currentConfig.accountType = 'offline';
      await window.fenixAPI.saveConfig(currentConfig);

      // 3. Выводим статус
      saveBtn.textContent = '✅ Сохранено!';
      if (authMsg) authMsg.textContent = `Ник сохранен: ${newNick}`;
      setTimeout(() => {
        saveBtn.textContent = 'Сохранить ник';
        if (authMsg) authMsg.textContent = '';
      }, 2000);
    });
  }

  // Лицензионный вход
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

// Проверка онлайн стрима Twitch
function checkTwitchStatus() {
  const pill = document.getElementById('twitch-status-pill');
  if (!pill) return;
  // Каждые 30 сек
  const check = async () => {
    try {
      const r = await fetch('https://static-cdn.jtvnw.net/previews-ttv/live_user_bloodrayt-320x180.jpg', { method: 'HEAD', cache: 'no-cache' });
      if (r.ok) pill.classList.remove('hidden');
    } catch(e) {}
  };
  check();
  setInterval(check, 30000);
}

// Инициализация при старте
document.addEventListener('DOMContentLoaded', async () => {
  setupTabs();
  setupProfile();
  renderShop();
  checkTwitchStatus();

  // Загружаем сохраненный конфиг
  try {
    const cfg = await window.fenixAPI.getConfig();
    if (cfg && cfg.username) {
      currentConfig = cfg;
      applyNickToUI(cfg.username, cfg.accountType === 'mojang' ? 'Лицензия' : 'Пиратка');
    }
  } catch(e) {}

  // Запуск игры
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
      }, 5000);
    });
  }

  // Кнопка папки игры
  const btnFolder = document.getElementById('btn-open-folder');
  if (btnFolder) {
    btnFolder.addEventListener('click', () => window.fenixAPI.openFolder(''));
  }
});
