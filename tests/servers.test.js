import assert from 'node:assert/strict';
import { test } from 'node:test';
import { autoJoinState, serverAddress, serversHtml, SERVER_DEFAULTS } from '../src/servers.js';

const [main, tech] = SERVER_DEFAULTS;

test('serverAddress adds the port only when it is not the standard one', () => {
  assert.equal(serverAddress(main), 'play.fenixcraft.ru');
  assert.equal(serverAddress({ ip: 'my.server.test', port: 25570 }), 'my.server.test:25570');
});

test('autoJoinState names the server the game will join', () => {
  const state = autoJoinState({ server: main, buildVersion: '26.3', enabled: true });

  assert.equal(state.enabled, true);
  assert.equal(state.serverName, 'FenixCraft');
  assert.equal(state.warning, '');
});

test('autoJoinState warns when the server runs another game version', () => {
  const state = autoJoinState({ server: tech, buildVersion: '26.3', enabled: true });

  assert.equal(state.warning, 'сервер на 1.21.1, игра откроет меню');
});

test('autoJoinState warns honestly when the server is offline', () => {
  const state = autoJoinState({ server: main, buildVersion: '26.3', enabled: true, online: false });

  assert.equal(state.offline, true);
  assert.equal(state.warning, 'сервер сейчас недоступен');
});

test('autoJoinState is off when disabled or nothing is selected', () => {
  assert.equal(autoJoinState({ server: main, buildVersion: '26.3', enabled: false }).enabled, false);
  assert.equal(autoJoinState({ server: undefined, buildVersion: '26.3', enabled: true }).enabled, false);
});

test('serversHtml escapes server data and marks online sensors', () => {
  const html = serversHtml(
    [{ ...main, name: '<b>x</b>' }],
    { 'fenix-main': { online: true, players: 3, maxPlayers: 100 } },
    main.ip,
  );

  assert.ok(html.includes('&lt;b&gt;x&lt;/b&gt;'));
  assert.ok(html.includes('sensor is-online'));
  assert.ok(html.includes('data-copy-ip="play.fenixcraft.ru"'));
});
