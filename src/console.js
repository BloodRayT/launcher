import { byId, invoke, tauriDialog } from './core.js';

const MAX_CONSOLE_LINES = 5000;

let consoleLines = [];
let consoleFlushScheduled = false;

function renderConsole() {
  consoleFlushScheduled = false;
  const output = byId('console-output');
  output.textContent = consoleLines.join('\n');
  output.scrollTop = output.scrollHeight;
}

export function appendConsole(line) {
  consoleLines.push(line);
  if (consoleLines.length > MAX_CONSOLE_LINES) consoleLines = consoleLines.slice(-MAX_CONSOLE_LINES);
  if (!consoleFlushScheduled) {
    consoleFlushScheduled = true;
    requestAnimationFrame(renderConsole);
  }
}

export function bindConsole() {
  byId('btn-clear-console').addEventListener('click', () => {
    consoleLines = [];
    renderConsole();
  });
  const copyButton = byId('btn-copy-console');
  copyButton.addEventListener('click', async () => {
    const selected = window.getSelection()?.toString();
    try {
      await navigator.clipboard.writeText(selected || consoleLines.join('\n'));
      const original = copyButton.textContent;
      copyButton.textContent = 'Скопировано';
      setTimeout(() => { copyButton.textContent = original; }, 2000);
    } catch (error) {
      appendConsole(`[Консоль] Не удалось скопировать: ${error}`);
    }
  });
  byId('btn-save-debug').addEventListener('click', async () => {
    const customPath = await tauriDialog?.save?.({
      title: 'Сохранить журнал лаунчера',
      defaultPath: 'fenixcraft-debug.log',
      filters: [{ name: 'Log Files', extensions: ['log', 'txt'] }],
    });
    try {
      const path = await invoke('save_debug_log', { consoleText: consoleLines.join('\n'), customPath: customPath ?? null });
      appendConsole(`[Консоль] Журнал сохранён: ${path}`);
    } catch (error) {
      appendConsole(`[Консоль] Не удалось сохранить журнал: ${error}`);
    }
  });
  byId('btn-verify-assets').addEventListener('click', () => {
    invoke('verify_assets_command').catch((error) => appendConsole(`[Проверка] Ошибка: ${error}`));
  });
}
