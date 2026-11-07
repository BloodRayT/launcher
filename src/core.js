/** Общие вызовы приложения: команды Tauri, ссылки и окно ошибки. */
import { safeLinkUrl } from './utils.js';
import { DEFAULT_JVM_ARGS } from './launch-options.js';

export const tauriCore = window.__TAURI__?.core;
export const tauriEvent = window.__TAURI__?.event;
export const tauriDialog = window.__TAURI__?.dialog;

/** Плащ 64×32 для предпросмотра: двухцветная полоса на внешней стороне. */
function previewCape(main, stripe) {
  const canvas = document.createElement('canvas');
  canvas.width = 64;
  canvas.height = 32;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = main;
  ctx.fillRect(0, 0, 22, 17);
  ctx.fillStyle = stripe;
  ctx.fillRect(1, 7, 10, 3);
  ctx.fillRect(12, 7, 10, 3);
  return canvas.toDataURL('image/png');
}

/** Вне приложения (проверка вёрстки в браузере) вызовы заменяются заглушками. */
const previewResponses = {
  get_config: () => ({
    username: 'Preview_Player', accountType: 'offline', ram: 6144, width: 1280, height: 720,
    javaPath: '', jvmArgs: DEFAULT_JVM_ARGS, showConsole: false, debugMode: false,
    selectedVersion: '26.2-iris-neoforge', selectedServer: 'play.fenixcraft.ru',
    customServers: [], avatarData: '', msUsername: '', msUuid: '', gameDirPath: '', skinData: '', autoJoin: true,
    customArgsEnabled: true, gameArgs: '',
  }),
  get_launcher_version: () => 'dev',
  get_game_dir: () => 'C:\\Users\\Player\\AppData\\Roaming\\.fenixcraft',
  get_connectivity: () => ({ internet: true, backend: true }),
  get_servers_status: () => ({
    'fenix-main': { online: true, players: 7 + Math.floor(Math.random() * 4), maxPlayers: 20 },
    'fenix-mods': { online: false, players: 0, maxPlayers: 500 },
  }),
  get_skin_catalog: () => ({
    skins: [
      { id: 'ice_boy', name: 'Ice boy', url: 'skins/ice-boy.png', slim: false },
      { id: 'ice_boy_slim', name: 'Ice boy (тонкий)', url: 'skins/ice-boy.png', slim: true },
    ],
    capes: [
      { id: 'phoenix', name: 'Феникс', url: previewCape('#ffff55', '#ff7a1a') },
      { id: 'emerald', name: 'Изумрудный', url: previewCape('#10b981', '#55ffff') },
    ],
  }),
  // Вход Ely.by в предпросмотре: пароль никуда не уходит, любой ник «входит» сразу.
  ely_login: ({ login }) => ({
    ...previewResponses.get_config(), accountType: 'ely', elyUsername: login.split('@')[0], elyUuid: '0'.repeat(32),
  }),
  ely_logout: () => previewResponses.get_config(),
  get_ely_textures: () => ({ username: 'Preview', skin: 'skins/ice-boy.png', cape: previewCape('#0f172a', '#18ff8c') }),
  select_skin: ({ skin, cape }) => ({ ...previewResponses.get_config(), skinChoice: skin || '', capeChoice: cape || '' }),
  list_minecraft_releases: () => ({
    latest: '26.3', offline: false,
    releases: ['26.3', '26.2', '26.1', '1.21.11', '1.21.10', '1.21.9', '1.21.8', '1.21.1', '1.21', '1.20.6', '1.20.4', '1.20.2', '1.20.1', '1.19.4', '1.18.2', '1.17.1', '1.16.5', '1.12.2', '1.8.9', '1.7.10'],
  }),
  get_vk_news: () => ({ success: false }),
  check_twitch_live: () => ({ live: false }),
  get_launcher_releases: () => [],
  check_launcher_update: () => ({ updateAvailable: false }),
  ms_login_available: () => false,
  save_launcher_config: (args) => args.newConfig,
};

export function invoke(command, args) {
  if (tauriCore) return tauriCore.invoke(command, args);
  return Promise.resolve(previewResponses[command]?.(args));
}

export const byId = (id) => document.getElementById(id);

const WINDOW_SIZE = Object.freeze({ width: 1280, height: 800 });

/**
 * Только для предпросмотра в браузере: лаунчер 1280×800 целиком уменьшается под окно, а не обрезается.
 * В приложении окно и так ровно 1280×800, поэтому там ничего не меняется.
 */
export function fitPreview() {
  if (tauriCore) return;
  const fit = () => {
    const scale = Math.min(1, window.innerWidth / WINDOW_SIZE.width, window.innerHeight / WINDOW_SIZE.height);
    document.body.style.zoom = String(scale);
  };
  fit();
  window.addEventListener('resize', fit);
}

export function showError({ title, message, hint }) {
  byId('error-title').textContent = title;
  byId('error-message').textContent = message;
  byId('error-hint').textContent = hint || '';
  byId('error-modal').classList.remove('hidden');
  byId('btn-error-close').focus();
}

export function hideError() {
  byId('error-modal').classList.add('hidden');
}

export function openUrl(url) {
  const safe = safeLinkUrl(url);
  if (!safe) return Promise.resolve();
  if (tauriCore) {
    return invoke('open_external_url', { url: safe }).catch((error) => showError({ title: 'Ссылка не открылась', message: String(error), hint: '' }));
  }
  window.open(safe, '_blank', 'noopener');
  return Promise.resolve();
}
