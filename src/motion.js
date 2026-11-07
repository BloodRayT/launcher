/** Настройки для слабых компьютеров: выключение всех анимаций и закрытие лаунчера после запуска игры. */
import { readNumber, writeNumber } from './storage.js';

const MOTION_OFF_KEY = 'fenix.motion.off';
const CLOSE_AFTER_LAUNCH_KEY = 'fenix.launch.close';
/** Через сколько закрывается лаунчер после запуска игры. */
export const CLOSE_AFTER_LAUNCH_MS = 5000;

/** @returns {boolean} выключены ли все анимации (по умолчанию нет) */
export function isMotionOff() {
  return readNumber(MOTION_OFF_KEY) === 1;
}

/** @param {boolean} off */
export function setMotionOff(off) {
  writeNumber(MOTION_OFF_KEY, off ? 1 : 0);
}

/** @returns {boolean} закрывать ли лаунчер после запуска игры (по умолчанию нет) */
export function isCloseAfterLaunch() {
  return readNumber(CLOSE_AFTER_LAUNCH_KEY) === 1;
}

/** @param {boolean} on */
export function setCloseAfterLaunch(on) {
  writeNumber(CLOSE_AFTER_LAUNCH_KEY, on ? 1 : 0);
}

/**
 * Включает или выключает все анимации разом. Свои выборы «Живой фон» и «Анимация» при этом не меняются.
 * @param {boolean} off
 * @param {{ wallpaper?: object, heroVideo?: object, embers?: object, skinStage?: object }} parts
 */
export function applyMotion(off, { wallpaper, heroVideo, embers, skinStage }) {
  document.body.classList.toggle('no-motion', off);
  wallpaper?.setSuspended(off);
  heroVideo?.setSuspended(off);
  embers?.setEnabled(!off);
  skinStage?.setMotion(!off);
}
