/** Аватарка игрока в профиле: картинка обрезается до квадрата и сжимается. */
import { byId } from './core.js';

const MAX_AVATAR_BYTES = 2 * 1024 * 1024;
const AVATAR_SIZE = 128;

function cropAvatar(image) {
  const canvas = document.createElement('canvas');
  canvas.width = AVATAR_SIZE;
  canvas.height = AVATAR_SIZE;
  const side = Math.min(image.width, image.height);
  canvas.getContext('2d').drawImage(image, (image.width - side) / 2, (image.height - side) / 2, side, side, 0, 0, AVATAR_SIZE, AVATAR_SIZE);
  return canvas.toDataURL('image/webp', 0.85);
}

/**
 * @param {{ current: string, save: (avatarData: string) => Promise<unknown>, notify: (text: string) => void }} options
 */
export function bindAvatar({ current, save, notify }) {
  const avatarImg = byId('sidebar-avatar-img');
  const avatarInput = byId('avatar-file-input');
  if (current) avatarImg.src = current;

  byId('avatar-container').addEventListener('click', () => avatarInput.click());
  avatarInput.addEventListener('change', (event) => {
    const file = event.target.files?.[0];
    if (!file) return;
    if (file.size > MAX_AVATAR_BYTES) {
      notify('Файл больше 2 МБ');
      return;
    }
    const reader = new FileReader();
    reader.onerror = () => notify('Не удалось открыть картинку');
    reader.onload = () => {
      const image = new Image();
      image.onerror = () => notify('Не удалось открыть картинку');
      image.onload = async () => {
        const avatarData = cropAvatar(image);
        avatarImg.src = avatarData;
        await save(avatarData);
        notify('Аватарка сохранена');
      };
      image.src = reader.result;
    };
    reader.readAsDataURL(file);
  });
}
