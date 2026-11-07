/** Настройки запуска на вкладке «Настройки»: свои аргументы и вид списка версий. */
import { readNumber, writeNumber } from './storage.js';

const SHOW_MODIFIED_KEY = 'fenix.versions.modified';

/** Стандартные аргументы JVM, как `DEFAULT_JVM_ARGS` в `src-tauri/core/src/config.rs`. */
export const DEFAULT_JVM_ARGS = '-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M';

/** @returns {boolean} показывать ли в списке версий Fabric, Forge и NeoForge */
export function readShowModified() {
  return readNumber(SHOW_MODIFIED_KEY) === 1;
}

/** @param {boolean} value */
export function writeShowModified(value) {
  writeNumber(SHOW_MODIFIED_KEY, value ? 1 : 0);
}

/**
 * Строка-превью выключенных аргументов: так они выглядят, как закомментированный код.
 * @param {string} jvmArgs @param {string} gameArgs @returns {string}
 */
export function commentedArgs(jvmArgs, gameArgs) {
  const lines = [jvmArgs.trim(), gameArgs.trim()].filter(Boolean);
  return lines.length > 0 ? lines.map((line) => `// ${line}`).join('\n') : '// аргументов нет';
}

/**
 * Включённые аргументы раскрыты и редактируются, выключенные затемнены, свёрнуты и недоступны.
 * @param {{ enabled: boolean, jvmArgs: string, gameArgs: string }} state
 */
export function renderCustomArgs({ enabled, jvmArgs, gameArgs }) {
  const block = document.getElementById('custom-args');
  block.classList.toggle('is-off', !enabled);
  block.setAttribute('aria-disabled', String(!enabled));
  for (const input of block.querySelectorAll('input')) input.disabled = !enabled;
  document.getElementById('custom-args-preview').textContent = enabled ? '' : commentedArgs(jvmArgs, gameArgs);
  document.getElementById('custom-args-state').textContent = enabled ? 'передаются игре' : 'выключены, игра их не получит';
}
