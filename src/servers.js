import { escapeHtml, serverStatusLabel } from './utils.js';

export const SERVER_DEFAULTS = Object.freeze([
  { id: 'fenix-main', name: 'FenixCraft', desc: 'Vanilla • 26.3', ip: 'play.fenixcraft.ru', port: 25565, icon: '🦅', maxDefault: 1000 },
  { id: 'fenix-mods', name: 'FenixTech', desc: 'NeoForge • 1.21.1', ip: 'tech.fenixcraft.ru', port: 25565, icon: '⚙️', maxDefault: 500 },
]);

/** @returns {string} разметка списка серверов в правой панели */
export function serversHtml(servers, statusById, selectedIp) {
  return servers.map((server) => {
    const status = statusById[server.id];
    const selected = selectedIp === server.ip;
    return `
      <div role="button" tabindex="0" data-server-ip="${escapeHtml(server.ip)}"
           class="glass-card p-2.5 rounded-xl flex items-center justify-between cursor-pointer transition ${selected ? 'border-emerald-500/80 bg-emerald-500/10' : 'hover:border-white/20'}">
        <div class="flex items-center gap-2.5 min-w-0">
          <div class="w-8 h-8 rounded-full bg-slate-800 flex items-center justify-center text-sm shrink-0">${escapeHtml(server.icon)}</div>
          <div class="min-w-0">
            <div class="text-xs font-bold text-white truncate">${escapeHtml(server.name)}</div>
            <div class="text-[10px] text-slate-400 truncate">${escapeHtml(server.desc)}</div>
          </div>
        </div>
        <div class="text-[11px] font-semibold flex items-center gap-1.5 shrink-0 pl-2 tabular-nums ${status?.online ? 'text-emerald-400' : 'text-slate-500'}">
          <span class="w-1.5 h-1.5 rounded-full ${status?.online ? 'bg-emerald-400' : 'bg-red-500'}"></span>
          <span>${escapeHtml(serverStatusLabel(status))}</span>
        </div>
      </div>`;
  }).join('');
}
