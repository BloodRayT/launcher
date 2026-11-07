/**
 * Живой баннер сборки: переключатель «Анимация» рядом с автовходом запускает ролик вместо картинки.
 * Есть только у сборок с полем video, по умолчанию выключен; выбор запоминается на этом компьютере.
 */
import { readNumber, writeNumber } from './storage.js';

const ANIMATION_KEY = 'fenix.banner.animation';

/** @returns {boolean} включена ли анимация баннера (по умолчанию нет) */
export function isBannerAnimationOn() {
  return readNumber(ANIMATION_KEY) === 1;
}

/**
 * @param {{ hero: HTMLElement, video: HTMLVideoElement, chip: HTMLButtonElement, dot: HTMLElement }} elements
 * @returns {{ update(build: { video?: string }): void }}
 */
export function bindHeroVideo({ hero, video, chip, dot }) {
  let on = isBannerAnimationOn();
  let visible = true;
  let suspended = false;
  let current = null;
  let source = '';

  const stop = () => {
    hero.classList.remove('is-animated');
    video.pause();
    if (video.getAttribute('src')) {
      // Выгруженный ролик не держит память и видеокарту.
      video.removeAttribute('src');
      video.load();
    }
    source = '';
  };

  const play = (url) => {
    if (source !== url) {
      source = url;
      video.src = url;
    }
    video.play().catch(() => hero.classList.remove('is-animated'));
  };

  const update = () => {
    const available = Boolean(current?.video);
    chip.classList.toggle('hidden', !available);
    chip.setAttribute('aria-pressed', String(on));
    dot.classList.toggle('is-on', on);
    if (available && on && visible && !suspended && !document.hidden) play(new URL(current.video, document.baseURI).href);
    else stop();
  };

  video.muted = true;
  video.loop = true;
  video.playsInline = true;
  // Картинка остаётся видна, пока ролик не начал играть.
  video.addEventListener('playing', () => hero.classList.add('is-animated'));
  video.addEventListener('error', () => hero.classList.remove('is-animated'));
  chip.addEventListener('click', () => {
    on = !on;
    writeNumber(ANIMATION_KEY, on ? 1 : 0);
    update();
  });
  // Свёрнутый лаунчер и другие вкладки не тратят ресурсы на ролик.
  document.addEventListener('visibilitychange', update);
  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    update();
  }).observe(hero);

  return {
    update(build) {
      current = build;
      update();
    },
    /** «Выключить все анимации» останавливает ролик, не меняя переключатель «Анимация». */
    setSuspended(off) {
      suspended = off;
      chip.disabled = off;
      update();
    },
  };
}
