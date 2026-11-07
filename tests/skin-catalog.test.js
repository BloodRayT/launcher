import assert from 'node:assert/strict';
import { test } from 'node:test';
import { catalogHtml, choiceToApply, frontParts, isElyChoice } from '../src/skin-catalog-view.js';
import { elyErrorText, isElySignedIn } from '../src/ely-account.js';

const CATALOG = {
  skins: [{ id: 'ice_boy', name: 'Ice <b>boy</b>', url: 'http://fenixcraft.ru/api/launcher/skins/skins/ice_boy.png' }],
  capes: [{ id: 'phoenix', name: 'Феникс', url: 'http://fenixcraft.ru/api/launcher/skins/capes/phoenix.png' }],
};

test('skin front view uses the modern layout and mirrors limbs for old 64x32 skins', () => {
  assert.equal(frontParts('skins', 64, 64).length, 12);
  assert.equal(frontParts('skins', 64, 32).length, 7);
  assert.deepEqual(frontParts('capes', 64, 32), [[1, 1, 10, 16, 0, 0]]);
});

test('catalog marks the current choice and escapes names from the server', () => {
  const html = catalogHtml(CATALOG, { skin: 'ice_boy', cape: '' }, 'skins');

  assert.ok(html.includes('data-pick-id="ice_boy" aria-pressed="true"'));
  assert.ok(html.includes('Ice &lt;b&gt;boy&lt;/b&gt;'));
  assert.ok(html.includes('Скины · 1'));
});

test('capes tab offers taking the cape off', () => {
  const html = catalogHtml(CATALOG, { skin: '', cape: '' }, 'capes');

  assert.ok(html.includes('data-pick-id="" aria-pressed="true"'));
  assert.ok(html.includes('Без плаща'));
  assert.ok(html.includes('data-pick-id="phoenix" aria-pressed="false"'));
});

test('empty catalog explains that skins are coming', () => {
  assert.ok(catalogHtml({ skins: [], capes: [] }, { skin: '', cape: '' }, 'skins').includes('Скинов пока нет'));
});

test('Ely.by tab asks to sign in until the player uses an Ely.by account', () => {
  const html = catalogHtml(CATALOG, { skin: '', cape: '' }, 'ely');

  assert.ok(html.includes('Войдите через Ely.by'));
  assert.ok(html.includes('data-skin-tab="ely" aria-selected="true"'));
});

test('Ely.by skin is chosen when no server skin or cape is worn', () => {
  const ely = { signedIn: true, username: 'Fenix', skin: 'data:image/png;base64,AA==', cape: '' };

  assert.equal(isElyChoice({ skin: '', cape: '' }, ely), true);
  assert.equal(isElyChoice({ skin: 'ice_boy', cape: '' }, ely), false);
  const html = catalogHtml(CATALOG, { skin: '', cape: '' }, 'ely', ely);
  assert.ok(html.includes('data-pick-kind="ely" data-pick-id="ely" aria-pressed="true"'));
  assert.ok(html.includes('https://ely.by/skins'));
  assert.ok(html.includes('Мой скин · Fenix'));
});

test('"Надеть" on the Ely.by tab wears the Ely.by skin even without clicking its card', () => {
  const serverChoice = { skin: '9bee0322cbcd9cb8', cape: 'ayunull' };

  assert.deepEqual(choiceToApply('ely', serverChoice, { signedIn: true }), { skin: '', cape: '' });
  assert.deepEqual(choiceToApply('skins', serverChoice, { signedIn: true }), serverChoice);
  assert.deepEqual(choiceToApply('ely', serverChoice, { signedIn: false }), serverChoice);
});

test('Ely.by login errors read like hints under the form', () => {
  assert.equal(elyErrorText({ code: 'ely_two_factor', message: 'x' }), 'Введите код из приложения-аутентификатора');
  assert.equal(elyErrorText({ code: 'ely_login_failed', message: 'Неверный пароль.' }), 'Неверный пароль.');
  assert.equal(isElySignedIn({ accountType: 'ely', elyUuid: 'abc' }), true);
  assert.equal(isElySignedIn({ accountType: 'offline', elyUuid: 'abc' }), false);
});
