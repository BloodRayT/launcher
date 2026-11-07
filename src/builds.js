/**
 * Сборки лаунчера. Идентификаторы совпадают с `PROFILES` в `src-tauri/core/src/config.rs`:
 * по ним ядро выбирает версию игры, загрузчик и шейдеры.
 * Любая другая версия из списка «Любая версия» описывается тем же форматом id: `1.20.1-vanilla`,
 * `1.20.1-iris` (Fabric с Iris), `1.20.4-iris-neoforge`, `1.20.1-forge`.
 */
export const BUILDS = Object.freeze([
  {
    id: '26.2-iris-neoforge', name: 'FenixCraft', version: '26.2', loader: 'neoforge', shaders: true,
    caption: '26.2 · NeoForge + Iris', banner: 'banners/fenixcraft.webp', video: 'banners/fenixcraft.mp4', backend: true,
    desc: 'Погрузись в новый мир ярких эмоций и приключений. Твоя уникальная история начинается здесь!',
  },
  {
    id: 'latest-vanilla', name: 'Vanilla', version: '', loader: 'vanilla', shaders: false,
    caption: 'Последняя версия', banner: 'banners/vanilla.webp',
    desc: 'Всегда последняя официальная версия Minecraft. Скачанная версия запускается и без интернета.',
  },
  {
    id: '1.21.11-iris', name: 'FenixClient', version: '1.21.11', loader: 'fabric', shaders: true,
    caption: '1.21.11 · Fabric + Iris', banner: 'banners/client-fabric.webp',
    desc: 'Лёгкий клиент на Fabric с Iris и Sodium: шейдеры и больше кадров в секунду.',
  },
  {
    id: '1.21.11-forge', name: 'FenixClient', version: '1.21.11', loader: 'forge', shaders: false,
    caption: '1.21.11 · Forge', banner: 'banners/client-forge.webp',
    desc: 'Клиент на Forge для модов, которые выходят только под Forge. Свои моды кладите в папку сборки.',
  },
  {
    id: '1.21.11-vanilla', name: 'FenixClient', version: '1.21.11', loader: 'vanilla', shaders: false,
    caption: '1.21.11 · Vanilla', banner: 'banners/client-vanilla.webp',
    desc: 'Чистая игра 1.21.11 без загрузчика модов. Исследуй. Строй. Выживай.',
  },
]);

export const BUILD_IDS = Object.freeze(BUILDS.map((build) => build.id));

export const LOADER_LABELS = Object.freeze({ vanilla: 'Vanilla', fabric: 'Fabric', forge: 'Forge', neoforge: 'NeoForge' });

/** Суффиксы id своей версии. Порядок важен: более длинные проверяются первыми. */
/** Совпадает с `CUSTOM_PROFILE_SUFFIXES` в config.rs, иначе сохранённая версия показывалась бы чужой сборкой. */
const CUSTOM_SUFFIXES = Object.freeze([
  { suffix: '-iris-neoforge', loader: 'neoforge', shaders: true },
  { suffix: '-iris-fabric', loader: 'fabric', shaders: true },
  { suffix: '-neoforge', loader: 'neoforge', shaders: false },
  { suffix: '-fabric', loader: 'fabric', shaders: false },
  { suffix: '-iris', loader: 'fabric', shaders: true },
  { suffix: '-forge', loader: 'forge', shaders: false },
  { suffix: '-vanilla', loader: 'vanilla', shaders: false },
]);

const LOADER_SUFFIX = Object.freeze({ vanilla: '-vanilla', fabric: '-iris', forge: '-forge', neoforge: '-iris-neoforge' });
const VERSION_PATTERN = /^\d+(?:\.\d+)+$/;

/** @param {string} version @param {'vanilla'|'fabric'|'forge'|'neoforge'} loader @returns {string} id сборки */
export function customProfileId(version, loader) {
  return `${version}${LOADER_SUFFIX[loader]}`;
}

/** @param {string} id @returns {null | { version: string, loader: string, shaders: boolean }} */
export function parseCustomProfile(id) {
  const text = String(id);
  const match = CUSTOM_SUFFIXES.find(({ suffix }) => text.endsWith(suffix));
  if (!match) return null;
  const version = text.slice(0, -match.suffix.length);
  return VERSION_PATTERN.test(version) ? { version, loader: match.loader, shaders: match.shaders } : null;
}

/** @param {{ version: string, loader: string, shaders: boolean }} parts @returns {string} «1.20.1 · Fabric + Iris» */
export function customCaption({ version, loader, shaders }) {
  return `${version} · ${LOADER_LABELS[loader]}${shaders ? ' + Iris' : ''}`;
}

/** @param {{ version: string, loader: string, shaders: boolean }} parts @returns {string} «Fabric 1.20.1 + Iris» */
export function customTitle({ version, loader, shaders }) {
  if (loader === 'vanilla') return `Версия ${version}`;
  return `${LOADER_LABELS[loader]} ${version}${shaders ? ' + Iris' : ''}`;
}

function customDesc({ version, loader, shaders }) {
  if (loader === 'vanilla') return `Официальная версия Minecraft ${version}. Скачивается при первом запуске.`;
  const extras = shaders ? ', Iris и Sodium (если они вышли для этой версии)' : '';
  return `Minecraft ${version} с ${LOADER_LABELS[loader]}${extras}. Устанавливается при первом запуске.`;
}

/**
 * @param {string} id
 * @returns {typeof BUILDS[number] & { custom?: boolean }} сборка, своя версия или первая сборка
 */
export function buildById(id) {
  const build = BUILDS.find((item) => item.id === id);
  if (build) return build;
  const parts = parseCustomProfile(id);
  if (!parts) return BUILDS[0];
  const sameLoader = BUILDS.find((item) => item.loader === parts.loader && !item.backend);
  return {
    id, ...parts, custom: true,
    name: customTitle(parts),
    caption: customCaption(parts),
    banner: (sameLoader ?? BUILDS[1]).banner,
    desc: customDesc(parts),
  };
}

/** @param {typeof BUILDS[number]} build @returns {string} короткая подпись для карточки сборки */
export function buildCaption(build) {
  return build.caption ?? customCaption(build);
}

/** @param {typeof BUILDS[number]} build @param {string} [latest] номер последней версии, если известен @returns {string[]} */
export function buildChips(build, latest = '') {
  const version = build.version || latest;
  const chips = [version ? `Minecraft ${version}` : 'Последний релиз', LOADER_LABELS[build.loader]];
  return build.shaders ? [...chips, 'Шейдеры Iris'] : chips;
}
