import { byId, invoke, showError } from './core.js';

/** Показывает окно обновления, если на GitHub вышла новая версия лаунчера. */
export async function checkForUpdates() {
  let update;
  try {
    update = await invoke('check_launcher_update');
  } catch {
    return;
  }
  if (!update?.updateAvailable) return;

  const modal = byId('update-modal');
  const headerButton = byId('btn-update-launcher');
  byId('update-version-label').textContent = `Доступна версия v${update.latestVersion}`;
  byId('update-changelog').textContent = update.changelog || 'Улучшена стабильность и исправлены ошибки.';
  headerButton.textContent = `Обновить до v${update.latestVersion}`;
  headerButton.classList.remove('hidden');
  headerButton.addEventListener('click', () => modal.classList.remove('hidden'));
  byId('btn-update-later').addEventListener('click', () => modal.classList.add('hidden'));
  const installButton = byId('btn-update-now');
  installButton.addEventListener('click', async () => {
    installButton.disabled = true;
    installButton.textContent = 'Загрузка…';
    try {
      await invoke('install_launcher_update', { downloadUrl: update.downloadUrl });
    } catch (error) {
      modal.classList.add('hidden');
      showError({ title: 'Обновление не установлено', message: String(error), hint: 'Попробуйте позже или скачайте лаунчер со страницы релизов.' });
    } finally {
      installButton.disabled = false;
      installButton.textContent = 'Обновить сейчас';
    }
  });
  modal.classList.remove('hidden');
}
