/**
 * Сборки лаунчера. Идентификаторы совпадают с `PROFILES` в `src-tauri/core/src/config.rs`:
 * по ним ядро выбирает версию игры, загрузчик и шейдеры.
 */
export const BUILDS = Object.freeze([
  {
    id: '26.3-vanilla', name: 'Vanilla', version: '26.3', loader: 'vanilla', shaders: false,
    desc: 'Чистая игра для основного сервера FenixCraft. Исследуй. Строй. Выживай.',
  },
  {
    id: '26.3-iris', name: 'Vanilla + Iris', version: '26.3', loader: 'fabric', shaders: true,
    desc: 'Та же ванильная игра с Iris и Sodium: шейдеры и больше кадров в секунду.',
  },
  {
    id: '1.21.1-neoforge', name: 'FenixTech', version: '1.21.1', loader: 'neoforge', shaders: false,
    desc: 'Сборка с модами для сервера FenixTech. Моды скачиваются с сервера сборок автоматически.',
  },
  {
    id: '1.21.1-iris-neoforge', name: 'FenixTech + Iris', version: '1.21.1', loader: 'neoforge', shaders: true,
    desc: 'Сборка FenixTech с Iris и Sodium для красивой картинки с шейдерами.',
  },
  {
    id: '26.2-iris-neoforge', name: 'NeoForge 26.2', version: '26.2', loader: 'neoforge', shaders: true, isNew: true,
    desc: 'Новая сборка с модами на Minecraft 26.2: NeoForge, Iris и Sodium. Моды раздаёт сервер сборок.',
  },
]);

export const BUILD_IDS = Object.freeze(BUILDS.map((build) => build.id));

const LOADER_LABELS = Object.freeze({ vanilla: 'Vanilla', fabric: 'Fabric', neoforge: 'NeoForge' });

/** @param {string} id @returns {typeof BUILDS[number]} сборка или первая, если такой нет */
export function buildById(id) {
  return BUILDS.find((build) => build.id === id) ?? BUILDS[0];
}

/** @param {typeof BUILDS[number]} build @returns {string} короткая подпись для карточки сборки */
export function buildCaption(build) {
  return [build.version, LOADER_LABELS[build.loader], ...(build.shaders ? ['Iris'] : [])].join(' · ');
}

/** @param {typeof BUILDS[number]} build @returns {string[]} подписи под названием сборки */
export function buildChips(build) {
  const chips = [`Minecraft ${build.version}`, LOADER_LABELS[build.loader]];
  return build.shaders ? [...chips, 'Шейдеры Iris'] : chips;
}
