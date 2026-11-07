/** Список «Любая версия»: релизы Minecraft и, по настройке, их варианты с загрузчиками модов. */
import { customProfileId, customTitle } from './builds.js';
import { escapeHtml } from './utils.js';

/** С какой версии игры есть загрузчик. Iris ставится по возможности: не для каждой версии он вышел. */
const LOADER_SINCE = Object.freeze({ fabric: '1.16.5', neoforge: '1.20.2', forge: '1.17.1' });
const MODIFIED_ORDER = Object.freeze(['fabric', 'forge', 'neoforge']);
const LOADER_BADGES = Object.freeze({ vanilla: 'MC', fabric: 'FB', forge: 'FG', neoforge: 'NF' });
const MAX_RENDERED = 400;

/** @returns {number} отрицательное, если a старше b: `1.9` < `1.21`, `1.21.11` < `26.1` */
export function compareVersions(a, b) {
  const left = String(a).split('.').map(Number);
  const right = String(b).split('.').map(Number);
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const diff = (left[index] ?? 0) - (right[index] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/** @param {string} version @param {'fabric'|'forge'|'neoforge'} loader @returns {boolean} */
export function hasLoader(version, loader) {
  return compareVersions(version, LOADER_SINCE[loader]) >= 0;
}

/**
 * @param {string[]} releases релизы от новых к старым
 * @param {boolean} showModified добавлять ли Fabric, Forge и NeoForge
 * @returns {{ id: string, title: string, loader: string, version: string }[]}
 */
export function versionEntries(releases, showModified) {
  return releases.flatMap((version) => {
    const vanilla = { id: customProfileId(version, 'vanilla'), title: `Версия ${version}`, loader: 'vanilla', version };
    if (!showModified) return [vanilla];
    const modified = MODIFIED_ORDER
      .filter((loader) => hasLoader(version, loader))
      .map((loader) => ({
        id: customProfileId(version, loader),
        title: customTitle({ version, loader, shaders: loader !== 'forge' }),
        loader,
        version,
      }));
    return [vanilla, ...modified];
  });
}

/** @param {ReturnType<typeof versionEntries>} entries @param {string} query @returns {typeof entries} */
export function filterEntries(entries, query) {
  const needle = query.trim().toLowerCase();
  if (!needle) return entries;
  return entries.filter((entry) => entry.title.toLowerCase().includes(needle));
}

function entryHtml(entry, selectedId) {
  const selected = entry.id === selectedId;
  return `
    <li role="option" tabindex="-1" aria-selected="${selected}" data-version-id="${escapeHtml(entry.id)}" class="version-option">
      <span class="version-option-badge loader-${entry.loader}" aria-hidden="true">${LOADER_BADGES[entry.loader]}</span>
      <span class="truncate">${escapeHtml(entry.title)}</span>
    </li>`;
}

function bindListKeys(list, search) {
  list.addEventListener('keydown', (event) => {
    const option = event.target.closest('[data-version-id]');
    if (!option) return;
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      option.click();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next = event.key === 'ArrowDown' ? option.nextElementSibling : option.previousElementSibling;
      (next ?? (event.key === 'ArrowUp' ? search : option)).focus();
    }
  });
  search.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      list.querySelector('[data-version-id]')?.focus();
    }
  });
}

/**
 * Подключает выпадающий список к разметке из index.html.
 * @param {{ loadReleases: () => Promise<{ releases: string[], offline: boolean }>, showModified: () => boolean,
 *           selectedId: () => string, onSelect: (id: string) => void }} options
 * @returns {{ refresh(): void, close(): void }}
 */
export function bindVersionPicker({ loadReleases, showModified, selectedId, onSelect }) {
  const button = document.getElementById('btn-version-picker');
  const menu = document.getElementById('version-menu');
  const list = document.getElementById('version-list');
  const search = document.getElementById('version-search');
  const note = document.getElementById('version-menu-note');
  let releases = [];
  let offline = false;

  const render = () => {
    const entries = filterEntries(versionEntries(releases, showModified()), search.value);
    list.innerHTML = entries.slice(0, MAX_RENDERED).map((entry) => entryHtml(entry, selectedId())).join('')
      || '<li class="version-empty">Ничего не найдено</li>';
    note.textContent = offline
      ? 'Нет интернета: показаны версии из сохранённого списка.'
      : `Релизов: ${releases.length}${showModified() ? ' · с Fabric, Forge и NeoForge' : ''}`;
  };

  const open = async () => {
    menu.classList.remove('hidden');
    button.setAttribute('aria-expanded', 'true');
    list.innerHTML = '<li class="version-empty">Загрузка списка версий…</li>';
    search.focus();
    try {
      ({ releases, offline } = await loadReleases());
    } catch {
      releases = [];
      offline = true;
    }
    render();
    list.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'center' });
  };

  const close = () => {
    if (menu.classList.contains('hidden')) return;
    menu.classList.add('hidden');
    button.setAttribute('aria-expanded', 'false');
  };

  button.addEventListener('click', () => (menu.classList.contains('hidden') ? open() : close()));
  search.addEventListener('input', render);
  list.addEventListener('click', (event) => {
    const option = event.target.closest('[data-version-id]');
    if (!option) return;
    close();
    onSelect(option.dataset.versionId);
    button.focus();
  });
  bindListKeys(list, search);
  menu.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    close();
    button.focus();
  });
  document.addEventListener('pointerdown', (event) => {
    if (!event.target.closest?.('.version-picker')) close();
  });

  return { refresh: () => !menu.classList.contains('hidden') && render(), close };
}
