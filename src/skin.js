/** 3D-модель персонажа в правой панели (skinview3d из src/vendor). */
import { isSkinSize } from './utils.js';

export const DEFAULT_SKIN = 'skins/ice-boy.png';

/** @param {File} file PNG со скином @returns {Promise<string>} data URL, если это текстура скина */
export function readSkinFile(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error('Файл не прочитан.'));
    reader.onload = () => {
      if (!String(reader.result).startsWith('data:image/png')) {
        reject(new Error('Это не PNG-картинка.'));
        return;
      }
      const image = new Image();
      image.onerror = () => reject(new Error('Это не PNG-картинка.'));
      image.onload = () => (isSkinSize(image.width, image.height)
        ? resolve(reader.result)
        : reject(new Error(`Картинка ${image.width}×${image.height}, а скин Minecraft — 64×64 или 64×32.`)));
      image.src = reader.result;
    };
    reader.readAsDataURL(file);
  });
}

const POSES = Object.freeze({
  idle: (lib) => new lib.IdleAnimation(),
  wave: (lib) => new lib.WaveAnimation(),
  walk: (lib) => new lib.WalkingAnimation(),
});

/**
 * @param {{ canvas: HTMLCanvasElement, fallback: HTMLElement }} elements
 * @returns {null | { setSkin(src: string): Promise<void>, setPose(name: string): void, setName(name: string): void }}
 *   null, если WebGL или библиотека недоступны: тогда показывается подсказка.
 */
export function createSkinStage({ canvas, fallback }) {
  const lib = window.skinview3d;
  const stage = canvas.parentElement;
  let viewer;
  try {
    viewer = new lib.SkinViewer({ canvas, width: stage.clientWidth || 240, height: stage.clientHeight || 300 });
  } catch {
    fallback.classList.remove('hidden');
    return null;
  }
  viewer.fov = 38;
  viewer.zoom = 0.7;
  viewer.autoRotate = true;
  viewer.autoRotateSpeed = 0.55;
  viewer.controls.enableZoom = false;
  viewer.controls.enablePan = false;
  viewer.animation = POSES.idle(lib);

  new ResizeObserver(() => viewer.setSize(stage.clientWidth, stage.clientHeight)).observe(stage);
  // Свёрнутое окно не должно тратить видеокарту на анимацию.
  document.addEventListener('visibilitychange', () => {
    viewer.renderPaused = document.hidden;
  });

  return {
    async setSkin(src) {
      await viewer.loadSkin(src, { model: 'auto-detect' });
    },
    setPose(name) {
      viewer.animation = (POSES[name] ?? POSES.idle)(lib);
    },
    setName(name) {
      // Ник над головой пиксельным шрифтом лаунчера; библиотека перерисует его, когда шрифт загрузится.
      viewer.nameTag = name ? new lib.NameTagObject(name, { font: '48px "Pixelify Sans"', textStyle: '#e8fff2' }) : null;
    },
  };
}
