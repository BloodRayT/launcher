/** Общие вызовы приложения: команды Tauri, ссылки и окно ошибки. */
import { safeLinkUrl } from './utils.js';

export const tauriCore = window.__TAURI__?.core;
export const tauriEvent = window.__TAURI__?.event;
export const tauriDialog = window.__TAURI__?.dialog;

/** Вне приложения (проверка вёрстки в браузере) вызовы заменяются заглушками. */
const previewResponses = {
  get_config: () => ({
    username: 'Preview_Player', accountType: 'offline', ram: 6144, width: 1280, height: 720,
    javaPath: '', jvmArgs: '-XX:+UseG1GC', showConsole: false, debugMode: false,
    selectedVersion: '26.2-iris-neoforge', selectedServer: 'play.fenixcraft.ru',
    customServers: [], avatarData: '', msUsername: '', msUuid: '', gameDirPath: '', skinData: '', autoJoin: true,
  }),
  get_launcher_version: () => 'dev',
  get_game_dir: () => 'C:\\Users\\Player\\AppData\\Roaming\\.fenixcraft',
  get_connectivity: () => ({ internet: true, backend: true }),
  get_servers_status: () => ({}),
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
