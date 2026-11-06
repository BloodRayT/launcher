import { escapeHtml } from './utils.js';

export const SHOP_URL = 'https://fenixcraft.trademc.org/';

/** Цвет привилегии выбирается только из этого списка классов. */
const SHOP_ITEMS = Object.freeze([
  {
    id: 'vip', tier: 'amber', name: 'Привилегия [VIP]', price: '149 ₽', badge: 'Хит продаж',
    perks: ['Префикс [VIP] в чате и табе', 'Полет /fly в лобби и приватных зонах', 'Доступ к киту /kit vip каждые 24ч', 'Вход на переполненный сервер', '5 дополнительных точек дома /sethome'],
  },
  {
    id: 'creativ', tier: 'emerald', name: 'Привилегия [Creative]', price: '299 ₽', badge: 'Строителям',
    perks: ['Режим креатива /gm 1 в разрешенных мирах', 'Доступ к командам WorldEdit (//set, //copy)', 'Префикс [Creative] и цветной чат', 'Эксклюзивный набор /kit creative', 'Все возможности привилегии VIP'],
  },
  {
    id: 'moder', tier: 'emerald', name: 'Привилегия [Moder]', price: '599 ₽', badge: 'Страж порядка',
    perks: ['Команды модерации (/mute, /kick, /tp)', 'Просмотр инвентаря игроков /invsee', 'Режим невидимости /vanish', 'Ежедневный бонус /kit moder', 'Иммунитет к кику и муту'],
  },
  {
    id: 'admin', tier: 'neon', name: 'Привилегия [Admin]', price: '999 ₽', badge: 'Максимум прав',
    perks: ['Полный набор прав [Admin] на сервере', 'Команды /ban, /tempban, /god, /heal', 'Управление погодой и временем в мире', 'Элитный кит /kit admin с топ-ресурсами', 'Включает все привилегии ниже'],
  },
  {
    id: 'money', tier: 'emerald', name: 'Игровая валюта [Money]', price: 'от 49 ₽', badge: 'Экономика',
    perks: ['Пакеты: 50 000$, 250 000$, 1 000 000$', 'Мгновенное зачисление на баланс /money', 'Покупка редких ресурсов на аукционе /ah', 'Создание собственных варпов и кланов', 'Бонус +25% при покупке от 250 000$'],
  },
]);

/** @param {string} buyerName ник покупателя @returns {string} разметка карточек магазина */
export function shopGridHtml(buyerName) {
  const buyer = escapeHtml(buyerName || 'Игрока');
  return SHOP_ITEMS.map((item) => `
    <div class="glass-card shop-card shop-${item.tier}">
      <div>
        <div class="flex items-start justify-between gap-2">
          <h3 class="shop-name">${item.name}</h3>
          <span class="shop-badge">${item.badge}</span>
        </div>
        <div class="shop-price tabular-nums">${item.price}</div>
        <ul class="shop-perks">
          ${item.perks.map((perk) => `<li>${perk}</li>`).join('')}
        </ul>
      </div>
      <button data-buy="${item.id}" class="shop-buy">Купить для ${buyer}</button>
    </div>
  `).join('');
}
