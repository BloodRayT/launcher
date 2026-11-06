import { escapeHtml } from './utils.js';

export const SHOP_URL = 'https://fenixcraft.trademc.org/';

const SHOP_ITEMS = Object.freeze([
  {
    id: 'vip', name: 'Привилегия [VIP]', price: '149 ₽', badge: 'Хит продаж', icon: '👑',
    color: 'from-amber-500/20 to-yellow-600/5 border-amber-500/40',
    btnColor: 'bg-amber-500 hover:bg-amber-400 text-slate-950',
    perks: ['Префикс [VIP] в чате и табе', 'Полет /fly в лобби и приватных зонах', 'Доступ к киту /kit vip каждые 24ч', 'Вход на переполненный сервер', '5 дополнительных точек дома /sethome'],
  },
  {
    id: 'creativ', name: 'Привилегия [Creative]', price: '299 ₽', badge: 'Строителям', icon: '🎨',
    color: 'from-cyan-500/20 to-blue-600/5 border-cyan-500/40',
    btnColor: 'bg-cyan-400 hover:bg-cyan-300 text-slate-950',
    perks: ['Режим креатива /gm 1 в разрешенных мирах', 'Доступ к командам WorldEdit (//set, //copy)', 'Префикс [Creative] и цветной чат', 'Эксклюзивный набор /kit creative', 'Все возможности привилегии VIP'],
  },
  {
    id: 'moder', name: 'Привилегия [Moder]', price: '599 ₽', badge: 'Страж порядка', icon: '🛡️',
    color: 'from-purple-500/20 to-indigo-600/5 border-purple-500/40',
    btnColor: 'bg-purple-500 hover:bg-purple-400 text-white',
    perks: ['Команды модерации (/mute, /kick, /tp)', 'Просмотр инвентаря игроков /invsee', 'Режим невидимости /vanish', 'Ежедневный бонус /kit moder', 'Иммунитет к кику и муту'],
  },
  {
    id: 'admin', name: 'Привилегия [Admin]', price: '999 ₽', badge: 'Максимум прав', icon: '⚡',
    color: 'from-rose-500/20 to-red-600/5 border-rose-500/40',
    btnColor: 'bg-rose-500 hover:bg-rose-400 text-white',
    perks: ['Полный набор прав [Admin] на сервере', 'Команды /ban, /tempban, /god, /heal', 'Управление погодой и временем в мире', 'Элитный кит /kit admin с топ-ресурсами', 'Включает все привилегии ниже'],
  },
  {
    id: 'money', name: 'Игровая валюта [Money]', price: 'от 49 ₽', badge: 'Экономика', icon: '💰',
    color: 'from-emerald-500/20 to-teal-600/5 border-emerald-500/40',
    btnColor: 'bg-emerald-500 hover:bg-emerald-400 text-slate-950',
    perks: ['Пакеты: 50 000$, 250 000$, 1 000 000$', 'Мгновенное зачисление на баланс /money', 'Покупка редких ресурсов на аукционе /ah', 'Создание собственных варпов и кланов', 'Бонус +25% при покупке от 250 000$'],
  },
]);

/** @param {string} buyerName ник покупателя @returns {string} разметка карточек магазина */
export function shopGridHtml(buyerName) {
  const buyer = escapeHtml(buyerName || 'Игрока');
  return SHOP_ITEMS.map((item) => `
    <div class="rounded-2xl p-4 bg-gradient-to-br ${item.color} border flex flex-col justify-between space-y-4">
      <div>
        <div class="flex items-center justify-between">
          <span class="text-2xl">${item.icon}</span>
          <span class="text-[10px] font-bold uppercase tracking-wider px-2.5 py-0.5 rounded-full bg-white/10 text-white">${item.badge}</span>
        </div>
        <h3 class="text-base font-extrabold text-white mt-2">${item.name}</h3>
        <div class="text-xl font-black text-emerald-400 mt-0.5">${item.price}</div>
        <ul class="text-xs text-slate-300 space-y-1.5 mt-3">
          ${item.perks.map((perk) => `<li class="flex items-start gap-1.5"><span class="text-emerald-400">✔</span><span>${perk}</span></li>`).join('')}
        </ul>
      </div>
      <button data-buy="${item.id}" class="w-full py-2.5 rounded-xl font-bold text-xs transition active:scale-95 ${item.btnColor}">Купить для ${buyer}</button>
    </div>
  `).join('');
}
