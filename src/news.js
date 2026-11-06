import { escapeHtml, formatReleaseDate, pickLocalImage, renderMarkdown, safeImageUrl, safeLinkUrl } from './utils.js';
import { SHOP_URL } from './shop.js';

export const LOCAL_NEWS_IMAGES = Object.freeze(['baner.webp', 'background.jpg']);
const RELEASES_PAGE = 'https://github.com/BloodRayT/launcher/releases';

export const FALLBACK_NEWS = Object.freeze([
  { tag: 'VK.RU/FENIXCRAFT', title: 'Сообщество FenixCraft ВКонтакте', text: 'Новости появятся здесь, как только лаунчер сможет связаться с ВКонтакте. Все записи и конкурсы есть на стене сообщества.', date: 'Официальная группа', image: '', images: [], url: 'https://vk.ru/fenixcraft' },
  { tag: 'Магазин', tone: 'amber', title: 'Привилегии VIP, Creative, Moder, Admin и Money', text: 'Поддержите развитие сервера и получите уникальные возможности в разделе «Магазин».', date: 'FenixCraft Shop', image: '', images: [], url: SHOP_URL },
]);

/** Классы выбираются только из этого списка: данные новостей приходят извне. */
const TAG_CLASSES = new Map([
  ['amber', 'tag-amber'],
  ['neon', 'tag-neon'],
]);

export function imageOf(post) {
  return safeImageUrl(post.image, pickLocalImage(post.title, LOCAL_NEWS_IMAGES));
}

function cardHtml({ attributes, image, tag, tagClass, title, text, date, fresh }) {
  return `
    <div role="button" tabindex="0" ${attributes} class="glass-card news-card${fresh ? ' is-fresh' : ''}">
      <div class="news-card-image">
        <img src="${escapeHtml(image)}" alt="" loading="lazy" class="news-image" />
        <span class="news-tag ${tagClass}">${escapeHtml(tag)}</span>
      </div>
      <div class="news-card-body">
        <div>
          <h3 class="text-sm font-bold text-white line-clamp-2">${escapeHtml(title)}${fresh ? '<span class="news-fresh-badge">Новое</span>' : ''}</h3>
          <p class="text-xs text-slate-300 mt-1 line-clamp-2">${escapeHtml(text)}</p>
        </div>
        <div class="text-[11px] text-emerald-300 flex justify-between items-center gap-3">
          <span class="truncate">${escapeHtml(date)}</span>
          <span class="shrink-0">Подробнее</span>
        </div>
      </div>
    </div>`;
}

/**
 * Две карточки главной: самая свежая запись сообщества и последнее обновление лаунчера.
 * @returns {{signature: string, html: string}[]}
 */
export function homeCards(posts, releases, freshIds) {
  const post = (posts.length > 0 ? posts : FALLBACK_NEWS)[0];
  const fresh = post.id !== undefined && freshIds.includes(post.id);
  const latest = releases[0];
  return [
    {
      signature: `post:${post.id ?? post.title}:${post.date}:${post.text?.length ?? 0}:${fresh}`,
      html: cardHtml({
        attributes: 'data-open-news="0"',
        image: imageOf(post),
        tag: post.tag || 'VK.RU/FENIXCRAFT',
        tagClass: TAG_CLASSES.get(post.tone) ?? 'tag-emerald',
        title: post.title || 'Новость FenixCraft',
        text: post.text,
        date: post.date,
        fresh,
      }),
    },
    {
      signature: `release:${latest?.version ?? 'none'}`,
      html: cardHtml({
        attributes: 'data-open-updates="true"',
        image: LOCAL_NEWS_IMAGES[0],
        tag: 'Лаунчер',
        tagClass: 'tag-neon',
        title: latest ? `Обновление ${latest.version}` : 'Обновления лаунчера',
        text: latest ? latest.body.replace(/[#*`]/g, '').trim() || latest.name : 'История версий появится, когда лаунчер свяжется с GitHub.',
        date: latest ? formatReleaseDate(latest.date) : 'GitHub',
        fresh: false,
      }),
    },
  ];
}

function articleHtml(post, index) {
  const images = (post.images?.length ? post.images : [post.image]).map((url) => safeImageUrl(url, '')).filter(Boolean);
  const shown = images.length > 0 ? images : [imageOf(post)];
  const url = safeLinkUrl(post.url);
  return `
    <article id="news-post-${index}" class="glass-article">
      <div class="glass-article-images ${shown.length > 1 ? 'is-gallery' : ''}">
        ${shown.map((src) => `<img src="${escapeHtml(src)}" alt="" loading="lazy" class="news-image" />`).join('')}
      </div>
      <div class="glass-article-meta">
        <span>${escapeHtml(post.date)}</span>
        ${url ? `<button data-external="${escapeHtml(url)}" class="glass-link">Открыть в браузере</button>` : ''}
      </div>
      <h4 class="glass-article-title">${escapeHtml(post.title)}</h4>
      <p class="glass-article-text selectable-console">${escapeHtml(post.text)}</p>
    </article>`;
}

export function newsArticlesHtml(posts) {
  return (posts.length > 0 ? posts : FALLBACK_NEWS).map(articleHtml).join('');
}

const TWITCH_CHANNEL = 'bloodrayt';
const TWITCH_URL = 'https://www.twitch.tv/bloodrayt';

/** @param {string} parent адрес страницы лаунчера: Twitch встраивается только с ним */
export function twitchHtml(parent) {
  return `
    <div class="twitch-layout">
      <iframe class="twitch-player" title="Плеер Twitch" allow="autoplay; fullscreen; picture-in-picture" allowfullscreen
              src="https://player.twitch.tv/?channel=${TWITCH_CHANNEL}&parent=${parent}&autoplay=true"></iframe>
      <iframe class="twitch-chat" title="Чат Twitch"
              src="https://www.twitch.tv/embed/${TWITCH_CHANNEL}/chat?parent=${parent}&darkpopout"></iframe>
    </div>
    <div class="glass-article-meta twitch-footer">
      <span>Плеер разворачивается на весь экран кнопкой в его правом нижнем углу</span>
      <button data-external="${TWITCH_URL}" class="glass-link">Открыть в браузере</button>
    </div>`;
}

export function releasesHtml(releases) {
  if (releases.length === 0) {
    return '<p class="glass-article-text">История обновлений пока недоступна. Проверьте подключение к интернету.</p>';
  }
  return releases.map((release) => `
    <article class="glass-article">
      <div class="glass-article-meta">
        <span>${escapeHtml(formatReleaseDate(release.date))}</span>
        <button data-external="${escapeHtml(safeLinkUrl(release.url) || RELEASES_PAGE)}" class="glass-link">Страница релиза</button>
      </div>
      <h4 class="glass-article-title">${escapeHtml(release.name)}</h4>
      <div class="glass-markdown selectable-console">${renderMarkdown(release.body) || '<p>Описание для этой версии не добавлено.</p>'}</div>
    </article>`).join('');
}
