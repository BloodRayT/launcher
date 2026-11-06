/**
 * Необязательный кеш интерфейса в localStorage. Любая ошибка хранилища
 * (приватный режим, переполнение) просто означает «кеша нет».
 */

/** @param {string} key @returns {unknown[]} сохранённый массив или пустой */
export function readList(key) {
  try {
    const cached = JSON.parse(localStorage.getItem(key));
    return Array.isArray(cached) ? cached : [];
  } catch {
    return [];
  }
}

/** @param {string} key @param {unknown[]} value */
export function writeList(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Кеш необязателен: без него покажутся карточки по умолчанию.
  }
}

/** @param {string} key @returns {number} сохранённое число или 0 */
export function readNumber(key) {
  try {
    return Number(localStorage.getItem(key)) || 0;
  } catch {
    return 0;
  }
}

/** @param {string} key @param {number} value */
export function writeNumber(key, value) {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // Без отметки данные просто запросятся ещё раз.
  }
}
