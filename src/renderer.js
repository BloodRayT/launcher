const tauriCore = window.__TAURI__?.core;
const tauriEvent = window.__TAURI__?.event;
const tauriDialog = window.__TAURI__?.dialog;

window.fenixAPI = {
  minimize: () => tauriCore?.invoke('window_minimize'),
  maximize: () => tauriCore?.invoke('window_maximize'),
  close: () => tauriCore?.invoke('window_close'),
  openUrl: (url) => tauriCore ? tauriCore.invoke('open_external_url', { url }) : window.open(url, '_blank'),
  getConfig: () => tauriCore ? tauriCore.invoke('get_config') : Promise.resolve({
    username: 'AndreyGusev', accountType: 'offline', ram: 4096, width: 1920, height: 1080,
    javaPath: '', jvmArgs: '-XX:+UseG1GC', showConsole: false, debugMode: false,
    selectedVersion: 'latest-vanilla', selectedServer: 'play.fenixcraft.ru', vkToken: ''
  }),
  saveConfig: (newConfig) => tauriCore ? tauriCore.invoke('save_config', { newConfig }) : Promise.resolve(newConfig),
  selectJava: async () => {
    if (!tauriDialog?.open) return null;
    return await tauriDialog.open({ multiple: false, directory: false, title: 'Выберите Java 21+ (java / javaw.exe)' });
  },
  getServersStatus: (servers) => tauriCore ? tauriCore.invoke('get_servers_status', { servers }) : Promise.resolve({}),
  checkTwitchLive: () => tauriCore ? tauriCore.invoke('check_twitch_live') : Promise.resolve({ live: false }),
  getVkNews: async () => {
    if (!tauriCore) return { success: false };
    const res = await tauriCore.invoke('get_vk_news');
    if (res && res.success) {
      if (Array.isArray(res.posts) && res.posts.length > 0) {
        return { success: true, posts: res.posts };
      }
      if (Array.isArray(res.data) && res.data.length > 0) {
        const posts = res.data.map(post => {
          const photo = post.attachments?.find(a => a.type === 'photo')?.photo?.sizes?.pop()?.url;
          const clean = (post.text || 'Новость сообщества FenixCraft').trim();
          return {
            tag: 'VK.RU/FENIXCRAFT',
            tagColor: 'bg-emerald-500 text-slate-950',
            title: clean.split('\n')[0].substring(0, 55) || 'FenixCraft ВК',
            text: clean,
            date: new Date(post.date * 1000).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }),
            image: photo || 'https://images.unsplash.com/photo-1607513746994-51f730a44832?w=600&q=80',
            url: `https://vk.ru/fenixcraft?w=wall${post.owner_id}_${post.id}`
          };
        });
        return { success: true, posts };
      }
    }
    return { success: false };
  },
  verifyAssets: () => tauriCore ? tauriCore.invoke('verify_assets_command') : Promise.resolve({ ok: true }),
  saveDebugLog: (consoleText, customPath) => tauriCore ? tauriCore.invoke('save_debug_log', { consoleText, customPath }) : Promise.resolve({ success: false }),
  launchGame: () => tauriCore ? tauriCore.invoke('launch_game') : Promise.resolve({ status: 'launched' }),
  onLog: (cb) => tauriEvent?.listen('game:log', (event) => cb(event.payload)),
  onProgress: (cb) => tauriEvent?.listen('game:progress', (event) => cb(event.payload)),
  onClosed: (cb) => tauriEvent?.listen('game:closed', () => cb())
};

const MY_SERVERS = [
  { id: 'fenix-main', name: 'FenixCraft', desc: 'Выживание • Vanilla/Mods', ip: 'play.fenixcraft.ru', port: 25565, icon: '🦅', maxDefault: 1000 },
  { id: 'fenix-mods', name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', port: 25565, icon: '⚙️', maxDefault: 500 },
  { id: 'fenix-rpg',  name: 'FenixRPG',  desc: 'Fabric + Iris • 26.3', ip: 'rpg.fenixcraft.ru', port: 25565, icon: '⚔️', maxDefault: 500 },
  { id: 'fenix-pvp',  name: 'FenixArena', desc: 'Анархия • 1.21.8', ip: 'pvp.fenixcraft.ru', port: 25565, icon: '⭐', maxDefault: 300 }
];

const SHOP_ITEMS = [
  {
    id: 'vip',
    name: 'Привилегия [VIP]',
    price: '149 ₽',
    badge: 'Хит продаж',
    color: 'from-amber-500/20 to-yellow-600/5 border-amber-500/40',
    btnColor: 'bg-amber-500 hover:bg-amber-400 text-slate-950',
    icon: '👑',
    perks: [
      'Префикс [VIP] в чате и табе',
      'Полет /fly в лобби и приватных зонах',
      'Доступ к киту /kit vip каждые 24ч',
      'Вход на переполненный сервер',
      '5 дополнительных точек дома /sethome'
    ]
  },
  {
    id: 'creativ',
    name: 'Привилегия [Creative]',
    price: '299 ₽',
    badge: 'Строителям',
    color: 'from-cyan-500/20 to-blue-600/5 border-cyan-500/40',
    btnColor: 'bg-cyan-400 hover:bg-cyan-300 text-slate-950',
    icon: '🎨',
    perks: [
      'Режим креатива /gm 1 в разрешенных мирах',
      'Доступ к командам WorldEdit (//set, //copy)',
      'Префикс [Creative] и цветной чат',
      'Эксклюзивный набор /kit creative',
      'Все возможности привилегии VIP'
    ]
  },
  {
    id: 'moder',
    name: 'Привилегия [Moder]',
    price: '599 ₽',
    badge: 'Страж порядка',
    color: 'from-purple-500/20 to-indigo-600/5 border-purple-500/40',
    btnColor: 'bg-purple-500 hover:bg-purple-400 text-white',
    icon: '🛡️',
    perks: [
      'Команды модерации (/mute, /kick, /tp)',
      'Просмотр инвентаря игроков /invsee',
      'Режим невидимости /vanish',
      'Ежедневный бонус /kit moder',
      'Иммунитет к кику и муту'
    ]
  },
  {
    id: 'admin',
    name: 'Привилегия [Admin]',
    price: '999 ₽',
    badge: 'Максимум прав',
    color: 'from-rose-500/20 to-red-600/5 border-rose-500/40',
    btnColor: 'bg-rose-500 hover:bg-rose-400 text-white',
    icon: '⚡',
    perks: [
      'Полный набор прав [Admin] на сервере',
      'Команды /ban, /tempban, /god, /heal',
      'Управление погодой и временем в мире',
      'Элитный кит /kit admin с топ-ресурсами',
      'Включает все привилегии ниже'
    ]
  },
  {
    id: 'money',
    name: 'Игровая валюта [Money]',
    price: 'от 49 ₽',
    badge: 'Экономика',
    color: 'from-emerald-500/20 to-teal-600/5 border-emerald-500/40',
    btnColor: 'bg-emerald-500 hover:bg-emerald-400 text-slate-950',
    icon: '💰',
    perks: [
      'Пакеты: 50 000$, 250 000$, 1 000 000$',
      'Мгновенное зачисление на баланс /money',
      'Покупка редких ресурсов на аукционе /ah',
      'Создание собственных варпов и кланов',
      'Бонус +25% при покупке от 250 000$'
    ]
  }
];

const FALLBACK_NEWS = [
  {
    tag: 'VK.RU/FENIXCRAFT',
    tagColor: 'bg-emerald-500 text-slate-950',
    title: 'Сообщество FenixCraft ВКонтакте',
    text: 'Нажмите на карточку, чтобы открыть официальную стену vk.ru/fenixcraft со всеми новостями и конкурсами!',
    date: 'Официальная группа',
    image: 'https://images.unsplash.com/photo-1607513746994-51f730a44832?w=600&q=80',
    url: 'https://vk.ru/fenixcraft'
  },
  {
    tag: 'Магазин',
    tagColor: 'bg-amber-500 text-slate-950',
    title: 'Привилегии VIP, Creative, Moder, Admin и Money',
    text: 'Поддержите развитие сервера и получите уникальные возможности в разделе «Магазин».',
    date: 'FenixCraft Shop',
    image: 'https://images.unsplash.com/photo-1518709268805-4e9042af9f23?w=600&q=80',
    url: 'https://fenixcraft.trademc.org/'
  },
  {
    tag: 'Лаунчер v1.0',
    tagColor: 'bg-cyan-500 text-slate-950',
    title: 'Автозагрузка Java 21 и Latest Vanilla',
    text: 'Лаунчер автоматически скачивает Java 21 (Adoptium) и последнюю версию игры напрямую с серверов Mojang.',
    date: 'Релиз v1.0',
    image: 'https://images.unsplash.com/photo-1534447677768-be436bb09401?w=600&q=80',
    url: 'https://vk.ru/fenixcraft'
  }
];

let currentConfig = {};

function switchTab(tabId) {
  if (tabId === 'play-now') {
    switchTab('home');
    document.getElementById('btn-play').click();
    return;
  }

  ['home', 'shop', 'servers', 'settings', 'faq', 'console'].forEach(id => {
    const el = document.getElementById(`tab-${id}`);
    if (el) el.classList.add('hidden');
  });
  const target = document.getElementById(`tab-${tabId}`) || document.getElementById('tab-home');
  target.classList.remove('hidden');

  document.querySelectorAll('.nav-btn').forEach(btn => {
    btn.classList.toggle('active-nav', btn.dataset.tab === tabId);
  });
}

document.querySelectorAll('.nav-btn').forEach(btn => {
  btn.addEventListener('click', () => switchTab(btn.dataset.tab));
});

document.getElementById('btn-top-settings').onclick = () => switchTab('settings');
document.getElementById('btn-quick-settings').onclick = () => switchTab('settings');
document.getElementById('btn-open-faq').onclick = () => switchTab('faq');
document.getElementById('btn-toggle-console').onclick = () => switchTab('console');

function renderShop() {
  const grid = document.getElementById('shop-grid');
  grid.innerHTML = SHOP_ITEMS.map(item => `
    <div class="rounded-2xl p-4 bg-gradient-to-br ${item.color} border flex flex-col justify-between space-y-4">
      <div>
        <div class="flex items-center justify-between">
          <span class="text-2xl">${item.icon}</span>
          <span class="text-[10px] font-bold uppercase tracking-wider px-2.5 py-0.5 rounded-full bg-white/10 text-white">${item.badge}</span>
        </div>
        <h3 class="text-base font-extrabold text-white mt-2">${item.name}</h3>
        <div class="text-xl font-black text-emerald-400 mt-0.5">${item.price}</div>
        <ul class="text-xs text-slate-300 space-y-1.5 mt-3">
          ${item.perks.map(p => `<li class="flex items-start space-x-1.5"><span class="text-emerald-400">✔</span><span>${p}</span></li>`).join('')}
        </ul>
      </div>
      <button onclick="buyPrivilege('${item.id}')" class="w-full py-2.5 rounded-xl font-bold text-xs transition active:scale-95 ${item.btnColor}">
        Купить для ${currentConfig.username || 'Игрока'}
      </button>
    </div>
  `).join('');
}

window.buyPrivilege = (itemId) => {
  const nick = encodeURIComponent(currentConfig.username || 'Player');
  window.fenixAPI.openUrl(`https://fenixcraft.trademc.org/?nickname=${nick}&item=${itemId}`);
};

function renderNews(posts) {
  const grid = document.getElementById('news-grid');
  grid.innerHTML = posts.slice(0, 3).map(p => `
    <div onclick="window.fenixAPI.openUrl('${p.url || 'https://vk.ru/fenixcraft'}')"
         class="glass-card rounded-xl overflow-hidden flex flex-col justify-between border border-white/5 hover:border-emerald-500/40 transition cursor-pointer">
      <div class="h-28 bg-cover bg-center relative" style="background-image: url('${p.image}')">
        <span class="absolute bottom-2 left-2 text-[10px] font-bold px-2.5 py-0.5 rounded-full ${p.tagColor || 'bg-emerald-500 text-slate-950'}">
          ${p.tag || 'VK.RU/FENIXCRAFT'}
        </span>
      </div>
      <div class="p-3.5 flex-1 flex flex-col justify-between space-y-2">
        <div>
          <h4 class="text-sm font-bold text-white line-clamp-1">${p.title || 'Новость FenixCraft'}</h4>
          <p class="text-xs text-slate-400 mt-1 line-clamp-2">${p.text}</p>
        </div>
        <div class="text-[11px] text-emerald-400/80 pt-1 flex justify-between items-center">
          <span>📅 ${p.date}</span>
          <span>Читать в ВК ↗</span>
        </div>
      </div>
    </div>
  `).join('');
}

async function loadVkNews() {
  const res = await window.fenixAPI.getVkNews();
  if (res.success && res.posts && res.posts.length > 0) {
    renderNews(res.posts);
  } else {
    renderNews(FALLBACK_NEWS);
  }
}


function getActiveServersList() {
  if (currentConfig && Array.isArray(currentConfig.customServers) && currentConfig.customServers.length > 0) {
    return currentConfig.customServers;
  }
  return MY_SERVERS;
}

window.addOrUpdateCustomServer = async () => {
  const name = document.getElementById('new-srv-name').value.trim();
  const desc = document.getElementById('new-srv-desc').value.trim() || 'Сервер FenixCraft';
  const ip = document.getElementById('new-srv-ip').value.trim();
  const port = parseInt(document.getElementById('new-srv-port').value, 10) || 25565;
  if (!name || !ip) return;

  const list = [...getActiveServersList()];
  const existingIdx = list.findIndex(s => s.ip.toLowerCase() === ip.toLowerCase() || s.name.toLowerCase() === name.toLowerCase());
  const item = {
    id: existingIdx >= 0 ? list[existingIdx].id : 'srv-' + Date.now(),
    name,
    desc,
    ip,
    port,
    icon: existingIdx >= 0 ? list[existingIdx].icon : '🦅',
    maxDefault: 1000
  };

  if (existingIdx >= 0) list[existingIdx] = item;
  else list.push(item);

  currentConfig = await window.fenixAPI.saveConfig({ ...currentConfig, customServers: list, selectedServer: ip });
  document.getElementById('new-srv-name').value = '';
  document.getElementById('new-srv-desc').value = '';
  document.getElementById('new-srv-ip').value = '';
  await refreshServers();
};

window.removeCustomServer = async (e, id) => {
  e.stopPropagation();
  const list = getActiveServersList().filter(s => s.id !== id);
  currentConfig = await window.fenixAPI.saveConfig({ ...currentConfig, customServers: list });
  await refreshServers();
};

window.resetDefaultServers = async () => {
  currentConfig = await window.fenixAPI.saveConfig({ ...currentConfig, customServers: MY_SERVERS });
  await refreshServers();
};


const btnCopyConsole = document.getElementById('btn-copy-console');
if (btnCopyConsole) {
  btnCopyConsole.onclick = async () => {
    const out = document.getElementById('console-output');
    const selected = window.getSelection()?.toString();
    const textToCopy = selected && selected.length > 0 ? selected : (out?.textContent || '');
    try {
      await navigator.clipboard.writeText(textToCopy);
      const oldText = btnCopyConsole.textContent;
      btnCopyConsole.textContent = '✅ Скопировано!';
      setTimeout(() => { btnCopyConsole.textContent = oldText; }, 2000);
    } catch (e) {}
  };
}

const btnSaveDebug = document.getElementById('btn-save-debug');
if (btnSaveDebug) {
  btnSaveDebug.onclick = async () => {
    const out = document.getElementById('console-output');
    const text = out?.textContent || '';
    let chosenPath = null;
    if (tauriDialog?.save) {
      try {
        chosenPath = await tauriDialog.save({
          title: 'Сохранить Debug-лог лаунчера',
          defaultPath: 'fenixcraft-debug.log',
          filters: [{ name: 'Log Files', extensions: ['log', 'txt'] }]
        });
      } catch (e) {}
    }
    const res = await window.fenixAPI.saveDebugLog(text, chosenPath);
    if (res && res.success) {
      out.textContent += `\n[ Debug Saved ] Лог успешно сохранен в файл: ${res.path}\n`;
      out.scrollTop = out.scrollHeight;
    }
  };
}

const btnVerifyAssets = document.getElementById('btn-verify-assets');
if (btnVerifyAssets) {
  btnVerifyAssets.onclick = async () => {
    await window.fenixAPI.verifyAssets();
  };
}

async function refreshServers() {
  const container = document.getElementById('servers-list');
  const fullContainer = document.getElementById('servers-full-list');
  const activeList = getActiveServersList();
  const statusMap = await window.fenixAPI.getServersStatus(activeList);

  const html = activeList.map(srv => {
    const st = statusMap[srv.id] || { online: false, players: 0, maxPlayers: srv.maxDefault };
    const isSelected = currentConfig.selectedServer === srv.ip;
    return `
      <div onclick="selectServer('${srv.ip}')"
           class="glass-card p-3 rounded-xl flex items-center justify-between cursor-pointer transition border ${isSelected ? 'border-emerald-500 bg-emerald-500/10' : 'border-white/5 hover:border-white/20'}">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-full bg-slate-800 flex items-center justify-center text-base">${srv.icon}</div>
          <div>
            <div class="text-xs font-bold text-white">${srv.name}</div>
            <div class="text-[10px] text-slate-400">${srv.desc} • ${srv.ip}</div>
          </div>
        </div>
        <div class="text-xs font-semibold flex items-center space-x-1.5 ${st.online ? 'text-emerald-400' : 'text-slate-500'}">
          <span class="w-2 h-2 rounded-full ${st.online ? 'bg-emerald-400' : 'bg-red-500'}"></span>
          <span>${st.players}/${st.maxPlayers}</span>
        </div>
      </div>
    `;
  }).join('');

  container.innerHTML = html;
  if (fullContainer) fullContainer.innerHTML = html;
}

window.selectServer = async (ip) => {
  currentConfig = await window.fenixAPI.saveConfig({ ...currentConfig, selectedServer: ip });
  refreshServers();
};

async function bindSettings() {
  currentConfig = await window.fenixAPI.getConfig();

  const elUser = document.getElementById('cfg-username');
  const elAccType = document.getElementById('cfg-account-type');
  const elRam = document.getElementById('cfg-ram');
  const elRamLabel = document.getElementById('ram-label');
  const elWidth = document.getElementById('cfg-width');
  const elHeight = document.getElementById('cfg-height');
  const elJava = document.getElementById('cfg-java');
  const elArgs = document.getElementById('cfg-args');
  const elConsole = document.getElementById('cfg-console');
  const elDebug = document.getElementById('cfg-debug');
  const elVersion = document.getElementById('select-version');

  elUser.value = currentConfig.username;
  elAccType.value = currentConfig.accountType;
  elRam.value = currentConfig.ram;
  elRamLabel.textContent = `${currentConfig.ram} МБ`;
  elWidth.value = currentConfig.width;
  elHeight.value = currentConfig.height;
  elJava.value = currentConfig.javaPath;
  elArgs.value = currentConfig.jvmArgs;
  elConsole.checked = currentConfig.showConsole;
  elDebug.checked = currentConfig.debugMode;
  const validVersions = ['latest-vanilla','1.21.11-vanilla','1.21.1-neoforge','1.21.11-fabric','1.21.11-iris','1.21.1-iris-neoforge','1.21.11-iris-fabric'];
  elVersion.value = validVersions.includes(currentConfig.selectedVersion) ? currentConfig.selectedVersion : 'latest-vanilla';

  document.getElementById('sidebar-username').textContent = currentConfig.username;
  document.getElementById('last-launch-ver').textContent = elVersion.options[elVersion.selectedIndex]?.text || 'Vanilla';

  const autoSave = async () => {
    elRamLabel.textContent = `${elRam.value} МБ`;
    currentConfig = await window.fenixAPI.saveConfig({
      username: elUser.value.trim() || 'Player',
      accountType: elAccType.value,
      ram: parseInt(elRam.value, 10),
      width: parseInt(elWidth.value, 10),
      height: parseInt(elHeight.value, 10),
      javaPath: elJava.value.trim(),
      jvmArgs: elArgs.value.trim(),
      showConsole: elConsole.checked,
      debugMode: elDebug.checked,
      selectedVersion: elVersion.value,
      selectedServer: currentConfig.selectedServer || 'play.fenixcraft.ru',
      vkToken: currentConfig.vkToken || ''
    });
    document.getElementById('sidebar-username').textContent = currentConfig.username;
    document.getElementById('sidebar-account-type').textContent =
      currentConfig.accountType === 'microsoft' ? 'Лицензия' : 'Онлайн';
    document.getElementById('last-launch-ver').textContent = elVersion.options[elVersion.selectedIndex]?.text || 'Vanilla';
    renderShop();
  };

  [elUser, elAccType, elRam, elWidth, elHeight, elJava, elArgs, elConsole, elDebug, elVersion].forEach(el => {
    el.addEventListener('input', autoSave);
    el.addEventListener('change', autoSave);
  });

  document.getElementById('btn-browse-java').onclick = async () => {
    const selected = await window.fenixAPI.selectJava();
    if (selected) {
      elJava.value = selected;
      autoSave();
    }
  };
}

document.getElementById('btn-play').addEventListener('click', async () => {
  const btn = document.getElementById('btn-play');
  const txt = document.getElementById('btn-play-text');
  const progBox = document.getElementById('progress-container');

  btn.disabled = true;
  txt.textContent = 'Скачивание...';
  progBox.classList.remove('hidden');

  if (currentConfig.showConsole) switchTab('console');

  const res = await window.fenixAPI.launchGame();
  if (res.status === 'error') {
    btn.disabled = false;
    txt.textContent = 'Играть';
  }
});

window.fenixAPI.onLog((msg) => {
  const out = document.getElementById('console-output');
  out.textContent += msg + '\n';
  out.scrollTop = out.scrollHeight;
});

window.fenixAPI.onProgress((prog) => {
  const percent = Math.round((prog.task / prog.total) * 100) || 0;
  document.getElementById('progress-status').textContent = `${prog.type} (${prog.task}/${prog.total})`;
  document.getElementById('progress-percent').textContent = `${percent}%`;
  document.getElementById('progress-bar').style.width = `${percent}%`;
});

window.fenixAPI.onClosed(() => {
  document.getElementById('btn-play').disabled = false;
  document.getElementById('btn-play-text').textContent = 'Играть';
  document.getElementById('progress-container').classList.add('hidden');
});

document.getElementById('btn-refresh-news').onclick = loadVkNews;
document.getElementById('btn-ping-servers').onclick = refreshServers;


// Проверка статуса стрима https://www.twitch.tv/bloodrayt
async function updateTwitchStatus() {
  try {
    const res = await window.fenixAPI.checkTwitchLive();
    const label = document.getElementById('twitch-label');
    const dot = document.getElementById('twitch-live-dot');
    if (!label || !dot) return;

    if (res && res.live) {
      label.classList.add('twitch-live-active');
      label.textContent = 'Twitch • LIVE';
      dot.classList.remove('hidden');
    } else {
      label.classList.remove('twitch-live-active');
      label.textContent = 'Twitch';
      dot.classList.add('hidden');
    }
  } catch (e) {}
}

(async () => {
  await bindSettings();
  renderShop();
  await loadVkNews();
  await refreshServers();
  await updateTwitchStatus();
  setInterval(updateTwitchStatus, 30000);
  setInterval(refreshServers, 30000);
})();
