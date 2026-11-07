/** Вход через Ely.by в блоке профиля: ник или почта, пароль и, если нужно, код подтверждения. */
export const ELY_REGISTER_URL = 'https://account.ely.by/register';
export const ELY_SKINS_URL = 'https://ely.by/skins';
const TWO_FACTOR_CODE = 'ely_two_factor';

const byId = (id) => document.getElementById(id);

/** @param {{ accountType?: string, elyUuid?: string }} config @returns {boolean} вошёл ли игрок через Ely.by */
export function isElySignedIn(config) {
  return config.accountType === 'ely' && Boolean(config.elyUuid);
}

/** @param {{ code?: string, title?: string, message?: string }} error @returns {string} короткая подпись под формой */
export function elyErrorText(error) {
  if (error?.code === TWO_FACTOR_CODE) return 'Введите код из приложения-аутентификатора';
  return error?.message || error?.title || String(error);
}

/** Форма видна, пока игрок не вошёл; после входа — его ник и кнопка «Выйти». */
export function renderElyBox(config) {
  const signedIn = isElySignedIn(config);
  byId('ely-form').classList.toggle('hidden', signedIn);
  byId('ely-signed').classList.toggle('hidden', !signedIn);
  byId('ely-signed-name').textContent = config.elyUsername || '';
}

/**
 * Пароль берётся из поля только на время запроса и сразу стирается.
 * @param {{ invoke: (command: string, args?: object) => Promise<any>,
 *           onChange: (config: object) => void, notify: (text: string) => void }} hooks
 */
export function bindElyAccount({ invoke, onChange, notify }) {
  const totpRow = byId('ely-totp-row');
  byId('ely-form').addEventListener('submit', async (event) => {
    event.preventDefault();
    const button = byId('btn-ely-login');
    const password = byId('ely-password');
    const login = byId('ely-login').value.trim();
    if (!login || !password.value) {
      notify('Введите ник или почту и пароль');
      return;
    }
    button.disabled = true;
    try {
      const config = await invoke('ely_login', {
        login,
        password: password.value,
        totp: byId('ely-totp').value.trim() || null,
      });
      password.value = '';
      byId('ely-totp').value = '';
      totpRow.classList.add('hidden');
      onChange(config);
      notify(`Вход выполнен: ${config.elyUsername}`);
    } catch (error) {
      if (error?.code === TWO_FACTOR_CODE) {
        totpRow.classList.remove('hidden');
        byId('ely-totp').focus();
      }
      notify(elyErrorText(error));
    } finally {
      button.disabled = false;
    }
  });
  byId('btn-ely-logout').addEventListener('click', async () => {
    try {
      onChange(await invoke('ely_logout'));
      notify('Вы вышли из Ely.by');
    } catch (error) {
      notify(elyErrorText(error));
    }
  });
}
