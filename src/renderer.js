const tauriCore = window.__TAURI__?.core;
const tauriEvent = window.__TAURI__?.event;
const tauriDialog = window.__TAURI__?.dialog;

window.fenixAPI = {
  minimize: () => tauriCore?.invoke('window_minimize'),
  maximize: () => tauriCore?.invoke('window_maximize'),
  close: () => tauriCore?.invoke('window_close'),
  getConfig: () => tauriCore ? tauriCore.invoke('get_config') : Promise.resolve({
    username: 'AndreyGusev', accountType: 'offline', ram: 4096, width: 1920, height: 1080,
    javaPath: '', jvmArgs: '-XX:+UseG1GC', showConsole: false, debugMode: false,
    selectedVersion: '1.21.8-neoforge', selectedServer: 'play.fenixcraft.ru'
  }),
  saveConfig: (newConfig) => tauriCore ? tauriCore.invoke('save_config', { newConfig }) : Promise.resolve(newConfig),
  selectJava: async () => {
    if (!tauriDialog?.open) return null;
    return await tauriDialog.open({ multiple: false, directory: false, title: 'Выберите Java 21+ (java / javaw.exe)' });
  },
  getServersStatus: (servers) => tauriCore ? tauriCore.invoke('get_servers_status', { servers }) : Promise.resolve({}),
  getVkNews: async () => {
    if (!tauriCore) return { success: false };
    const res = await tauriCore.invoke('get_vk_news');
    if (res && res.success && Array.isArray(res.data)) {
      const posts = res.data.map(post => {
        const photo = post.attachments?.find(a => a.type === 'photo')?.photo?.sizes?.pop()?.url;
        return {
          title: 'Новость FenixCraft',
          text: post.text || 'Обновление на сервере!',
          date: new Date(post.date * 1000).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }),
          image: photo || 'https://images.unsplash.com/photo-1607513746994-51f730a44832?w=600&q=80'
        };
      });
      return { success: true, posts };
    }
    return { success: false };
  },
  launchGame: () => tauriCore ? tauriCore.invoke('launch_game') : Promise.resolve({ status: 'launched' }),
  onLog: (cb) => tauriEvent?.listen('game:log', (event) => cb(event.payload)),
  onProgress: (cb) => tauriEvent?.listen('game:progress', (event) => cb(event.payload)),
  onClosed: (cb) => tauriEvent?.listen('game:closed', () => cb())
};

const MY_SERVERS = [
  { id: 'fenix-main', name: 'FenixCraft', desc: 'Выживание • 1.21.8', ip: 'play.fenixcraft.ru', port: 25565, icon: '🦅', maxDefault: 1000 },
  { id: 'fenix-mods', name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', port: 25565, icon: '⚙️', maxDefault: 500 },
  { id: 'fenix-rpg',  name: 'FenixRPG',  desc: 'Fabric + Iris • 26.3', ip: 'rpg.fenixcraft.ru', port: 25565, icon: '⚔️', maxDefault: 500 },
  { id: 'fenix-pvp',  name: 'FenixArena', desc: 'Анархия • 1.21.8', ip: 'pvp.fenixcraft.ru', port: 25565, icon: '⭐', maxDefault: 300 }
];

const FALLBACK_NEWS = [
  {
    tag: 'Обновление',
    tagColor: 'bg-emerald-500 text-slate-950',
    title: 'Вышел Minecraft 1.21.8 и поддержка 26.3!',
    text: 'Новые блоки, оптимизация рендера Iris и полная поддержка NeoForge на серверах FenixCraft.',
    date: '28 сентября 2026',
    image: 'https://images.unsplash.com/photo-1607513746994-51f730a44832?w=600&q=80'
  },
  {
    tag: 'Моды',
    tagColor: 'bg-purple-500 text-white',
    title: 'Лучшие моды для 1.21.1 — 1.21.8',
    text: 'Подборка самых популярных индустриальных и магических модов нашего проекта.',
    date: '26 сентября 2026',
    image: 'https://images.unsplash.com/photo-1518709268805-4e9042af9f23?w=600&q=80'
  },
  {
    tag: 'Лаунчер',
    tagColor: 'bg-cyan-500 text-slate-950',
    title: 'Обновление защиты SHA-256 и Java 21',
    text: 'Лаунчер автоматически проверяет целостность клиента и докачивает нужные библиотеки.',
    date: '24 сентября 2026',
    image: 'https://images.unsplash.com/photo-1534447677768-be436bb09401?w=600&q=80'
  }
];

let currentConfig = {};

function switchTab(tabId) {
  ['home', 'settings', 'faq', 'console'].forEach(id => {
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
  btn.addEventListener('click', () => {
    const tab = btn.dataset.tab === 'mods' || btn.dataset.tab === 'servers' ? 'home' : btn.dataset.tab;
    switchTab(tab);
  });
});

document.getElementById('btn-top-settings').onclick = () => switchTab('settings');
document.getElementById('btn-quick-settings').onclick = () => switchTab('settings');
document.getElementById('btn-open-faq').onclick = () => switchTab('faq');
document.getElementById('btn-toggle-console').onclick = () => switchTab('console');

function renderNews(posts) {
  const grid = document.getElementById('news-grid');
  grid.innerHTML = posts.slice(0, 3).map(p => `
    <div class="glass-card rounded-xl overflow-hidden flex flex-col justify-between border border-white/5 hover:border-emerald-500/30 transition">
      <div class="h-28 bg-cover bg-center relative" style="background-image: url('${p.image}')">
        <span class="absolute bottom-2 left-2 text-[10px] font-bold px-2.5 py-0.5 rounded-full ${p.tagColor || 'bg-emerald-500 text-slate-950'}">
          ${p.tag || 'ВКонтакте'}
        </span>
      </div>
      <div class="p-3.5 flex-1 flex flex-col justify-between space-y-2">
        <div>
          <h4 class="text-sm font-bold text-white line-clamp-1">${p.title || 'Новость FenixCraft'}</h4>
          <p class="text-xs text-slate-400 mt-1 line-clamp-2">${p.text}</p>
        </div>
        <div class="text-[11px] text-slate-500 pt-1">📅 ${p.date}</div>
      </div>
    </div>
  `).join('');
}

async function loadVkNews() {
  const res = await window.fenixAPI.getVkNews();
  if (res.success && res.posts.length > 0) {
    renderNews(res.posts);
  } else {
    renderNews(FALLBACK_NEWS);
  }
}

async function refreshServers() {
  const container = document.getElementById('servers-list');
  const statusMap = await window.fenixAPI.getServersStatus(MY_SERVERS);

  container.innerHTML = MY_SERVERS.map(srv => {
    const st = statusMap[srv.id] || { online: false, players: 0, maxPlayers: srv.maxDefault };
    const isSelected = currentConfig.selectedServer === srv.ip;
    return `
      <div onclick="selectServer('${srv.ip}')"
           class="glass-card p-3 rounded-xl flex items-center justify-between cursor-pointer transition border ${isSelected ? 'border-emerald-500 bg-emerald-500/10' : 'border-white/5 hover:border-white/20'}">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-full bg-slate-800 flex items-center justify-center text-base">${srv.icon}</div>
          <div>
            <div class="text-xs font-bold text-white">${srv.name}</div>
            <div class="text-[10px] text-slate-400">${srv.desc}</div>
          </div>
        </div>
        <div class="text-xs font-semibold flex items-center space-x-1.5 ${st.online ? 'text-emerald-400' : 'text-slate-500'}">
          <span class="w-2 h-2 rounded-full ${st.online ? 'bg-emerald-400' : 'bg-red-500'}"></span>
          <span>${st.players}/${st.maxPlayers}</span>
        </div>
      </div>
    `;
  }).join('');
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
  elVersion.value = currentConfig.selectedVersion;

  document.getElementById('sidebar-username').textContent = currentConfig.username;
  document.getElementById('last-launch-ver').textContent = currentConfig.selectedVersion.split('-')[0];

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
      selectedServer: currentConfig.selectedServer || 'play.fenixcraft.ru'
    });
    document.getElementById('sidebar-username').textContent = currentConfig.username;
    document.getElementById('sidebar-account-type').textContent =
      currentConfig.accountType === 'microsoft' ? 'Лицензия' : 'Онлайн';
    document.getElementById('last-launch-ver').textContent = currentConfig.selectedVersion.split('-')[0];
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
  txt.textContent = 'Запуск...';
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
  document.getElementById('progress-status').textContent = `Загрузка файлов (${prog.type}): ${prog.task}/${prog.total}`;
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

(async () => {
  await bindSettings();
  await loadVkNews();
  await refreshServers();
  setInterval(refreshServers, 30000);
})();