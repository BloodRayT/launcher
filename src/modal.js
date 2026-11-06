/** Окно поверх лаунчера: новости, обновления и Twitch. */
import { byId } from './core.js';

let opener = null;
let kind = '';

/** Пока открыто окно, основной интерфейс недоступен и для клавиатуры. */
function setBackgroundInert(inert) {
  document.querySelectorAll('body > header, #app-shell').forEach((element) => {
    element.inert = inert;
  });
}

/** @returns {string} что показано в окне: 'news', 'updates', 'twitch' или пусто */
export function openModalKind() {
  return kind;
}

export function openGlassModal({ title, html, wide = false, kind: modalKind = '' }) {
  opener = document.activeElement;
  kind = modalKind;
  byId('glass-modal-title').textContent = title;
  byId('glass-modal-body').innerHTML = html;
  byId('glass-modal-body').scrollTop = 0;
  byId('glass-modal-panel').classList.toggle('glass-panel-wide', wide);
  byId('glass-modal').classList.remove('hidden');
  setBackgroundInert(true);
  byId('btn-glass-close').focus();
}

/** Заменяет содержимое открытого окна, не сбрасывая прокрутку. */
export function replaceModalBody(html) {
  const body = byId('glass-modal-body');
  const scroll = body.scrollTop;
  body.innerHTML = html;
  body.scrollTop = scroll;
}

export function closeGlassModal() {
  if (byId('glass-modal').classList.contains('hidden')) return;
  byId('glass-modal').classList.add('hidden');
  kind = '';
  setBackgroundInert(false);
  // Удаление содержимого останавливает плеер и звук трансляции.
  byId('glass-modal-body').innerHTML = '';
  opener?.focus?.();
}
