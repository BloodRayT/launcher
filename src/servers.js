import { escapeHtml, serverStatusLabel } from './utils.js';

const DEFAULT_PORT = 25565;

export const SERVER_DEFAULTS = Object.freeze([
  { id: 'fenix-main', name: 'FenixCraft', desc: 'Vanilla • 26.3', ip: 'play.fenixcraft.ru', port: DEFAULT_PORT, icon: '🦅', maxDefault: 1000 },
  { id: 'fenix-mods', name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', port: DEFAULT_PORT, icon: '⚙️', maxDefault: 500 },
]);

/** @returns {string} адрес для чата игры: порт пишется, только если он не стандартный */
export function serverAddress(server) {
  const port = Number(server.port) || DEFAULT_PORT;
  return port === DEFAULT_PORT ? server.ip : `${server.ip}:${port}`;
}

/**
 * Что покажет переключатель автовхода. Версия сервера берётся из его описания,
 * например «NeoForge • 1.21.1»; при расхождении со сборкой игрок видит предупреждение.
 */
export function autoJoinState({ server, buildVersion, enabled, online = true }) {
  if (!enabled || !server) return { enabled: false, offline: false, serverName: '', warning: '' };
  if (!online) return { enabled: true, offline: true, serverName: server.name, warning: 'сервер сейчас недоступен' };
  const serverVersion = String(server.desc ?? '').match(/\d+(?:\.\d+)+/)?.[0] ?? '';
  const mismatch = serverVersion && serverVersion !== buildVersion;
  return { enabled: true, offline: false, serverName: server.name, warning: mismatch ? `сервер на ${serverVersion}, игра откроет меню` : '' };
}

/** @returns {string} разметка списка серверов в правой панели */
export function serversHtml(servers, statusById, selectedIp) {
  return servers.map((server) => {
    const status = statusById[server.id];
    const online = Boolean(status?.online);
    const selected = selectedIp === server.ip;
    return `
      <div data-server-id="${escapeHtml(server.id)}" class="glass-card server-card${selected ? ' is-selected' : ''}">
        <button type="button" data-server-ip="${escapeHtml(server.ip)}" aria-pressed="${selected}" class="server-select">
          <span class="sensor${online ? ' is-online' : ''}" aria-hidden="true"></span>
          <span class="min-w-0 block">
            <span class="server-name block truncate">${escapeHtml(server.name)}</span>
            <span class="text-[11px] text-slate-400 block truncate">${escapeHtml(server.desc)}</span>
          </span>
        </button>
        <div class="flex flex-col items-end gap-1 shrink-0">
          <span class="server-status tabular-nums${online ? ' is-online' : ''}">${escapeHtml(serverStatusLabel(status))}</span>
          <button type="button" class="copy-ip" data-copy-ip="${escapeHtml(serverAddress(server))}" title="Скопировать адрес сервера">IP</button>
        </div>
      </div>`;
  }).join('');
}
