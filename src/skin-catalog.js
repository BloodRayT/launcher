/** Окно «Выбрать скин»: скины и плащи из каталога сервера, выбор уходит в игру через CustomSkinLoader. */
import { byId } from './core.js';
import { closeGlassModal, openGlassModal, openModalKind } from './modal.js';
import { escapeHtml } from './utils.js';
import { catalogHtml, frontParts } from './skin-catalog-view.js';

const MODAL_KIND = 'skins';
let openings = 0;

function drawPreviews(root) {
  root.querySelectorAll('canvas[data-texture]').forEach((canvas) => {
    const image = new Image();
    image.onload = () => {
      const ctx = canvas.getContext('2d');
      const scale = image.width / 64;
      for (const [sx, sy, w, h, dx, dy] of frontParts(canvas.dataset.kind, image.width, image.height)) {
        ctx.drawImage(image, sx * scale, sy * scale, w * scale, h * scale, dx, dy, w, h);
      }
    };
    image.src = canvas.dataset.texture;
  });
}

/**
 * @param {{ load: () => Promise<{ skins: object[], capes: object[] }>,
 *           apply: (choice: { skin: string, cape: string }) => Promise<void>,
 *           current: { skin: string, cape: string },
 *           ely: { signedIn: boolean, load: (force: boolean) => Promise<{ username: string, skin?: string, cape?: string }> } }} options
 */
export async function openSkinCatalog({ load, apply, current, ely }) {
  openGlassModal({ title: 'Скины и плащи FenixCraft', html: '<p class="skin-catalog-empty">Загрузка каталога…</p>', kind: MODAL_KIND });
  // Окно могли закрыть и открыть снова, пока грузился каталог: рисует только последнее открытие.
  const opening = ++openings;
  const isCurrent = () => opening === openings && openModalKind() === MODAL_KIND;
  const body = byId('glass-modal-body');
  let catalog;
  try {
    catalog = await load();
  } catch (error) {
    if (isCurrent()) body.innerHTML = `<p class="skin-catalog-empty">${escapeHtml(String(error))}</p>`;
    return;
  }
  if (!isCurrent()) return;
  catalog = { skins: catalog?.skins ?? [], capes: catalog?.capes ?? [] };
  const choice = { ...current };
  let tab = 'skins';
  let elyState = { signedIn: Boolean(ely?.signedIn), loading: false };
  const render = () => {
    body.innerHTML = catalogHtml(catalog, choice, tab, elyState);
    drawPreviews(body);
  };
  // Скин с Ely.by загружается при первом открытии вкладки и по кнопке «Обновить».
  const loadEly = async (force) => {
    if (!elyState.signedIn) return;
    elyState = { ...elyState, loading: true };
    render();
    const textures = await ely.load(force).catch(() => ({}));
    if (!isCurrent()) return;
    elyState = { signedIn: true, loading: false, ...textures };
    render();
  };
  const status = (text, isError = false) => {
    const line = byId('skin-catalog-status');
    line.textContent = text;
    line.classList.toggle('is-error', isError);
  };

  body.onclick = async (event) => {
    // Обработчик живёт на общем окне: в окнах новостей и Twitch он ничего не делает.
    if (!isCurrent()) return;
    const tabButton = event.target.closest('[data-skin-tab]');
    if (tabButton) {
      tab = tabButton.dataset.skinTab;
      render();
      if (tab === 'ely' && elyState.signedIn && !('username' in elyState)) await loadEly(false);
      return;
    }
    if (event.target.closest('[data-ely-refresh]')) {
      await loadEly(true);
      return;
    }
    const card = event.target.closest('[data-pick-id]');
    if (card) {
      const kind = card.dataset.pickKind;
      // Скин с Ely.by — это отказ от скина сервера: игра возьмёт текстуры с Ely.by.
      if (kind === 'ely') Object.assign(choice, { skin: '', cape: '' });
      else choice[kind === 'skins' ? 'skin' : 'cape'] = card.dataset.pickId;
      render();
      return;
    }
    if (!event.target.closest('#btn-skin-apply')) return;
    const button = byId('btn-skin-apply');
    button.disabled = true;
    status('Сохраняем выбор на сервере…');
    try {
      await apply(choice);
      closeGlassModal();
    } catch (error) {
      status(String(error), true);
      button.disabled = false;
    }
  };
  render();
}
