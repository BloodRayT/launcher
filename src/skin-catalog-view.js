/** Разметка окна «Выбрать скин» и раскладка превью. Без обращения к окну браузера, поэтому проверяется тестами. */
import { escapeHtml } from './utils.js';

/** Кусочки текстуры 64×64 для вида спереди 16×32: [x, y, ширина, высота, куда x, куда y]. */
const SKIN_FRONT = Object.freeze([
  [8, 8, 8, 8, 4, 0], [40, 8, 8, 8, 4, 0],
  [20, 20, 8, 12, 4, 8], [20, 36, 8, 12, 4, 8],
  [44, 20, 4, 12, 0, 8], [44, 36, 4, 12, 0, 8],
  [36, 52, 4, 12, 12, 8], [52, 52, 4, 12, 12, 8],
  [4, 20, 4, 12, 4, 20], [4, 36, 4, 12, 4, 20],
  [20, 52, 4, 12, 8, 20], [4, 52, 4, 12, 8, 20],
]);
/** У старых скинов 64×32 левые рука и нога — копия правых. */
const LEGACY_FRONT = Object.freeze([
  [8, 8, 8, 8, 4, 0], [40, 8, 8, 8, 4, 0],
  [20, 20, 8, 12, 4, 8], [44, 20, 4, 12, 0, 8], [44, 20, 4, 12, 12, 8],
  [4, 20, 4, 12, 4, 20], [4, 20, 4, 12, 8, 20],
]);
/** Внешняя сторона плаща на текстуре 64×32. */
const CAPE_FRONT = Object.freeze([[1, 1, 10, 16, 0, 0]]);

/** @param {'skins'|'capes'} kind @param {number} width @param {number} height @returns {readonly number[][]} */
export function frontParts(kind, width, height) {
  if (kind === 'capes') return CAPE_FRONT;
  return width === height * 2 ? LEGACY_FRONT : SKIN_FRONT;
}

function cardHtml(kind, item, selected) {
  const size = kind === 'skins' ? 'width="16" height="32"' : 'width="10" height="16"';
  const preview = item.url
    ? `<canvas class="skin-card-preview is-${kind}" ${size} data-texture="${escapeHtml(item.url)}" data-kind="${kind}"></canvas>`
    : '<span class="skin-card-empty" aria-hidden="true">—</span>';
  return `
    <button type="button" class="skin-card" data-pick-kind="${kind}" data-pick-id="${escapeHtml(item.id)}" aria-pressed="${selected}">
      ${preview}
      <span class="skin-card-name">${escapeHtml(item.name)}</span>
    </button>`;
}

const ELY_SKINS_URL = 'https://ely.by/skins';
const ELY_REGISTER_URL = 'https://account.ely.by/register';

/** Свой скин и плащ с Ely.by выбираются вместе: тогда скин сервера снимается и игра берёт их с Ely.by. */
export function isElyChoice(choice, ely) {
  return Boolean(ely?.signedIn) && !choice.skin && !choice.cape;
}

function elyPreview(kind, url) {
  const size = kind === 'skins' ? 'width="16" height="32"' : 'width="10" height="16"';
  return url
    ? `<canvas class="skin-card-preview is-${kind}" ${size} data-texture="${escapeHtml(url)}" data-kind="${kind}"></canvas>`
    : '';
}

function elyPanelHtml(choice, ely) {
  if (!ely?.signedIn) {
    return `
      <div class="ely-panel">
        <p class="skin-catalog-empty">Войдите через Ely.by в блоке профиля (вкладка «Ely.by»), чтобы носить скины с сайта ely.by.</p>
        <button type="button" data-external="${ELY_REGISTER_URL}" class="btn-outline text-xs px-4 py-2">Регистрация на Ely.by</button>
      </div>`;
  }
  const body = ely.loading
    ? '<p class="skin-catalog-empty">Загружаем скин с Ely.by…</p>'
    : `
      <button type="button" class="skin-card ely-card" data-pick-kind="ely" data-pick-id="ely" aria-pressed="${isElyChoice(choice, ely)}">
        <span class="ely-card-previews">
          ${elyPreview('skins', ely.skin) || '<span class="skin-card-empty" aria-hidden="true">—</span>'}
          ${elyPreview('capes', ely.cape)}
        </span>
        <span class="skin-card-name">Мой скин · ${escapeHtml(ely.username || '')}</span>
      </button>`;
  return `
    <div class="ely-panel">
      <div class="skin-grid">${body}</div>
      <div class="ely-actions">
        <button type="button" data-external="${ELY_SKINS_URL}" class="btn-outline text-xs px-4 py-2">Каталог скинов на ely.by</button>
        <button type="button" data-ely-refresh class="btn-ghost text-xs px-4 py-2">Обновить</button>
      </div>
      <p class="ely-hint">Выберите скин в каталоге на сайте ely.by, вернитесь сюда, нажмите «Обновить», а затем «Надеть».</p>
    </div>`;
}

function serverGridHtml(catalog, choice, tab) {
  const items = tab === 'skins'
    ? catalog.skins
    : [{ id: '', name: 'Без плаща', url: '' }, ...catalog.capes];
  const selectedId = tab === 'skins' ? choice.skin : choice.cape;
  const grid = items.length > 0
    ? items.map((item) => cardHtml(tab, item, item.id === selectedId)).join('')
    : '<p class="skin-catalog-empty">Скинов пока нет. Администратор добавит их на сервер.</p>';
  return `<div class="skin-grid">${grid}</div>`;
}

/**
 * @param {{ skins: object[], capes: object[] }} catalog
 * @param {{ skin: string, cape: string }} choice выбранные id, пустая строка — ничего
 * @param {'skins'|'capes'|'ely'} tab
 * @param {{ signedIn: boolean, username?: string, skin?: string, cape?: string, loading?: boolean }} [ely]
 * @returns {string}
 */
export function catalogHtml(catalog, choice, tab, ely = { signedIn: false }) {
  const content = tab === 'ely' ? elyPanelHtml(choice, ely) : serverGridHtml(catalog, choice, tab);
  const note = tab === 'ely'
    ? 'Скин с Ely.by видят в игре все, у кого стоит CustomSkinLoader.'
    : 'Скин и плащ появятся в игре во всех сборках с модами.';
  return `
    <div class="skin-catalog">
      <div class="skin-tabs" role="tablist" aria-label="Что выбрать">
        <button type="button" role="tab" data-skin-tab="skins" aria-selected="${tab === 'skins'}">Скины · ${catalog.skins.length}</button>
        <button type="button" role="tab" data-skin-tab="capes" aria-selected="${tab === 'capes'}">Плащи · ${catalog.capes.length}</button>
        <button type="button" role="tab" data-skin-tab="ely" aria-selected="${tab === 'ely'}">Ely.by</button>
      </div>
      ${content}
      <footer class="skin-catalog-footer">
        <p id="skin-catalog-status" class="skin-catalog-status" aria-live="polite">${note}</p>
        <button type="button" id="btn-skin-apply" class="btn-primary text-sm px-6 py-2.5">Надеть</button>
      </footer>
    </div>`;
}
