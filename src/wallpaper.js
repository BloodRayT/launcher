/**
 * Живой фон из ролика игрока (background.mp4): крутится по кругу без звука.
 * Галочка «Живой фон» выключает его: видео останавливается и выгружается, остаётся картинка background.jpg.
 */
const VIDEO_URL = 'background.mp4';
const LIVE_KEY = 'fenix.background.live';

/** @returns {boolean} включён ли живой фон (по умолчанию да) */
export function isLiveWallpaperOn() {
  try {
    return localStorage.getItem(LIVE_KEY) !== '0';
  } catch {
    return true;
  }
}

/** @param {boolean} value */
export function setLiveWallpaper(value) {
  try {
    localStorage.setItem(LIVE_KEY, value ? '1' : '0');
  } catch {
    // Без хранилища настройка просто не запомнится.
  }
}

/**
 * @param {HTMLVideoElement} video
 * @returns {{ setLive(on: boolean): void }}
 */
export function startWallpaper(video) {
  let live = false;
  let suspended = false;

  const play = () => {
    video.play().catch(() => {
      // Если ролик не запустился, просто остаётся картинка.
      video.classList.remove('is-ready');
    });
  };

  const turnOn = () => {
    live = true;
    if (suspended) return;
    if (!video.getAttribute('src')) {
      video.src = VIDEO_URL;
      video.load();
    }
    play();
  };

  const turnOff = () => {
    live = false;
    video.pause();
    video.classList.remove('is-ready');
    // Выгруженный ролик не занимает память и видеокарту.
    video.removeAttribute('src');
    video.load();
  };

  video.muted = true;
  video.loop = true;
  video.playsInline = true;
  video.addEventListener('playing', () => video.classList.add('is-ready'));
  video.addEventListener('error', () => video.classList.remove('is-ready'));
  // Свёрнутый лаунчер не тратит ресурсы на видео.
  document.addEventListener('visibilitychange', () => {
    if (!live || suspended) return;
    if (document.hidden) video.pause();
    else play();
  });

  if (isLiveWallpaperOn()) turnOn();

  return {
    setLive(on) {
      setLiveWallpaper(on);
      if (on) turnOn();
      else turnOff();
    },
    /** «Выключить все анимации»: ролик останавливается, а выбор игрока остаётся. */
    setSuspended(off) {
      suspended = off;
      const wanted = isLiveWallpaperOn();
      if (off) turnOff();
      else if (wanted) turnOn();
      live = wanted;
    },
  };
}
