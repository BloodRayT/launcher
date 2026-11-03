const HTML_ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

/** @param {unknown} value @returns {string} текст, безопасный для вставки в HTML и атрибуты */
export function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>"']/g, (char) => HTML_ESCAPES[char]);
}

/** @param {unknown} raw @param {string[]} protocols @returns {string} адрес или пустая строка */
function allowedUrl(raw, protocols) {
  try {
    const url = new URL(String(raw));
    return protocols.includes(url.protocol) ? url.href : '';
  } catch {
    return '';
  }
}

/** Ссылки из новостей могут вести только на обычные веб-страницы. */
export function safeLinkUrl(raw) {
  return allowedUrl(raw, ['https:', 'http:']);
}

/** Внешние картинки загружаются только по HTTPS, локальные файлы указываются напрямую. */
export function safeImageUrl(raw, fallback) {
  return allowedUrl(raw, ['https:']) || fallback;
}

/** @param {string} name @returns {boolean} соответствует ли ник правилам Minecraft */
export function isValidUsername(name) {
  return /^[A-Za-z0-9_]{3,16}$/.test(name);
}

/** @param {{online: boolean, players: number, maxPlayers: number} | undefined} status */
export function serverStatusLabel(status) {
  return status?.online ? `${status.players}/${status.maxPlayers}` : 'Недоступен';
}

/** @param {{internet: boolean, backend: boolean}} state */
export function connectivityLabel(state) {
  if (!state.internet) {
    return { text: 'Без сети', tone: 'offline', hint: 'Установленные сборки запускаются и без интернета.' };
  }
  if (!state.backend) {
    return { text: 'Сервер сборок недоступен', tone: 'degraded', hint: 'Используем сохранённые файлы сборок.' };
  }
  return { text: 'Онлайн', tone: 'online', hint: 'Интернет и сервер сборок доступны.' };
}

