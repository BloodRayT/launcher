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
    return { text: 'Без сети', tone: 'offline', hint: 'Ванильные версии, скачанные ранее, запускаются без интернета. Сборкам с модами нужен сервер сборок.' };
  }
  if (!state.backend) {
    return { text: 'Технические работы', tone: 'degraded', hint: 'Сервер сборок недоступен. Сборки с модами временно не запускаются, ванильные версии работают.' };
  }
  return { text: 'Онлайн', tone: 'online', hint: 'Интернет и сервер сборок доступны.' };
}


/** Безопасный разбор описания релиза: сначала экранирование, затем только простая разметка. */
export function renderMarkdown(source) {
  const inline = (line) => escapeHtml(line)
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`(.+?)`/g, '<code>$1</code>');
  const html = [];
  let inList = false;
  for (const raw of String(source ?? '').replace(/\r/g, '').split('\n')) {
    const line = raw.trim();
    const item = line.match(/^[-*]\s+(.*)$/);
    if (!item && inList) {
      html.push('</ul>');
      inList = false;
    }
    if (item) {
      if (!inList) html.push('<ul>');
      inList = true;
      html.push(`<li>${inline(item[1])}</li>`);
    } else if (line.startsWith('#')) {
      html.push(`<h4>${inline(line.replace(/^#+\s*/, ''))}</h4>`);
    } else if (line) {
      html.push(`<p>${inline(line)}</p>`);
    }
  }
  if (inList) html.push('</ul>');
  return html.join('');
}

/** @param {string} iso дата из GitHub @returns {string} дата вида 05.11.2026 */
export function formatReleaseDate(iso) {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleDateString('ru-RU');
}

/**
 * Позиции карточек, содержимое которых изменилось. Перерисовываются только они,
 * поэтому лента не мигает и картинки не загружаются заново.
 * @param {string[]} previous подписи прежних карточек @param {string[]} next подписи новых
 * @returns {number[]}
 */
export function changedSlots(previous, next) {
  return next.flatMap((signature, index) => (previous[index] === signature ? [] : [index]));
}

/**
 * Записи, которых не было при прошлой проверке. При первой загрузке новыми ничего не считается.
 * @param {{id: unknown}[]} posts @param {unknown[]} seenIds @returns {unknown[]}
 */
export function unseenPostIds(posts, seenIds) {
  if (seenIds.length === 0) return [];
  const seen = new Set(seenIds);
  return posts.map((post) => post.id).filter((id) => !seen.has(id));
}

/** @param {number} width @param {number} height @returns {boolean} подходит ли картинка как скин Minecraft */
export function isSkinSize(width, height) {
  return width === 64 && (height === 64 || height === 32);
}

/** Запасная картинка новости: один и тот же пост всегда получает одну и ту же. */
export function pickLocalImage(seed, images) {
  const hash = [...String(seed ?? '')].reduce((sum, char) => (sum * 31 + char.codePointAt(0)) % 9973, 7);
  return images[hash % images.length];
}
