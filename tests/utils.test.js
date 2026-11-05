import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  connectivityLabel,
  escapeHtml,
  formatReleaseDate,
  isValidUsername,
  pickLocalImage,
  renderMarkdown,
  safeImageUrl,
  safeLinkUrl,
  serverStatusLabel,
} from '../src/utils.js';

test('escapeHtml neutralises markup and attribute breakouts', () => {
  assert.equal(
    escapeHtml(`<img src=x onerror="alert('1')">&`),
    '&lt;img src=x onerror=&quot;alert(&#39;1&#39;)&quot;&gt;&amp;',
  );
  assert.equal(escapeHtml(null), '');
  assert.equal(escapeHtml(5), '5');
});

test('safeLinkUrl only allows web pages', () => {
  assert.equal(safeLinkUrl('https://vk.ru/fenixcraft'), 'https://vk.ru/fenixcraft');
  for (const bad of ['javascript:alert(1)', 'file:///C:/calc.exe', 'ms-settings:', 'not a url', '', null]) {
    assert.equal(safeLinkUrl(bad), '', `accepted ${bad}`);
  }
});

test('safeImageUrl falls back unless the image is https', () => {
  assert.equal(safeImageUrl('https://sun.vk/a.jpg', 'local.webp'), 'https://sun.vk/a.jpg');
  for (const bad of ['http://sun.vk/a.jpg', 'javascript:alert(1)', '', undefined]) {
    assert.equal(safeImageUrl(bad, 'local.webp'), 'local.webp', `accepted ${bad}`);
  }
});

test('isValidUsername follows Minecraft rules', () => {
  assert.ok(isValidUsername('Fenix_123'));
  for (const bad of ['', 'ab', 'seventeen_chars__', 'Игрок', 'with space', 'a-b']) {
    assert.equal(isValidUsername(bad), false, `accepted ${bad}`);
  }
});

test('serverStatusLabel does not show a fake player count for offline servers', () => {
  assert.equal(serverStatusLabel({ online: true, players: 3, maxPlayers: 100 }), '3/100');
  assert.equal(serverStatusLabel({ online: false, players: 0, maxPlayers: 100 }), 'Недоступен');
  assert.equal(serverStatusLabel(undefined), 'Недоступен');
});

test('connectivityLabel separates no internet from a backend outage', () => {
  assert.equal(connectivityLabel({ internet: false, backend: false }).tone, 'offline');
  assert.equal(connectivityLabel({ internet: true, backend: false }).tone, 'degraded');
  assert.equal(connectivityLabel({ internet: true, backend: true }).tone, 'online');
});


test('renderMarkdown formats simple markup and escapes everything else', () => {
  const html = renderMarkdown('## Версия 1\n- **важно** и `код`\n- второй\n\nТекст <img src=x onerror=alert(1)>');

  assert.equal(
    html,
    '<h4>Версия 1</h4><ul><li><strong>важно</strong> и <code>код</code></li><li>второй</li></ul><p>Текст &lt;img src=x onerror=alert(1)&gt;</p>',
  );
  assert.equal(renderMarkdown(null), '');
});

test('formatReleaseDate tolerates missing dates', () => {
  assert.equal(formatReleaseDate('2026-11-05T10:00:00Z'), new Date('2026-11-05T10:00:00Z').toLocaleDateString('ru-RU'));
  assert.equal(formatReleaseDate(''), '');
});

test('pickLocalImage is stable and always returns a listed image', () => {
  const images = ['a.webp', 'b.jpg'];

  assert.equal(pickLocalImage('пост', images), pickLocalImage('пост', images));
  assert.ok(images.includes(pickLocalImage('', images)));
  assert.ok(images.includes(pickLocalImage(null, images)));
});
