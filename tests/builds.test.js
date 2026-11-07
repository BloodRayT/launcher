import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { BUILD_IDS, buildById, buildCaption, buildChips, customProfileId, parseCustomProfile } from '../src/builds.js';
import { commentedArgs, DEFAULT_JVM_ARGS } from '../src/launch-options.js';
import { compareVersions, filterEntries, hasLoader, versionEntries } from '../src/versions.js';

test('launcher builds match the profiles the Rust core accepts, in the same order', () => {
  const config = readFileSync(new URL('../src-tauri/core/src/config.rs', import.meta.url), 'utf8');
  const block = config.match(/pub const PROFILES: \[&str; \d+\] = \[([\s\S]*?)\];/)[1];
  const rustProfiles = [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);

  assert.deepEqual(BUILD_IDS, rustProfiles);
});

test('FenixCraft comes first and runs NeoForge 26.2 with Iris from the build server', () => {
  const build = buildById(BUILD_IDS[0]);

  assert.equal(build.name, 'FenixCraft');
  assert.equal(build.backend, true);
  assert.equal(buildCaption(build), '26.2 · NeoForge + Iris');
  assert.deepEqual(buildChips(build), ['Minecraft 26.2', 'NeoForge', 'Шейдеры Iris']);
});

test('Vanilla always follows the latest release', () => {
  const vanilla = buildById('latest-vanilla');

  assert.deepEqual(buildChips(vanilla), ['Последний релиз', 'Vanilla']);
  assert.deepEqual(buildChips(vanilla, '26.3'), ['Minecraft 26.3', 'Vanilla']);
});

test('every built-in build has its own banner', () => {
  const banners = BUILD_IDS.map((id) => buildById(id).banner);
  assert.equal(new Set(banners).size, banners.length);
});

test('any release from the version list is described like a build', () => {
  assert.deepEqual(parseCustomProfile('1.20.1-iris'), { version: '1.20.1', loader: 'fabric', shaders: true });
  assert.deepEqual(parseCustomProfile('1.20.4-iris-neoforge'), { version: '1.20.4', loader: 'neoforge', shaders: true });
  assert.equal(parseCustomProfile('1.20.1-quilt'), null);
  assert.deepEqual(parseCustomProfile('1.20.4-neoforge'), { version: '1.20.4', loader: 'neoforge', shaders: false });
  assert.deepEqual(parseCustomProfile('1.20.1-iris-fabric'), { version: '1.20.1', loader: 'fabric', shaders: true });
  assert.deepEqual(parseCustomProfile('1.20.1-fabric'), { version: '1.20.1', loader: 'fabric', shaders: false });
  assert.equal(parseCustomProfile('../1.2-vanilla'), null);

  const custom = buildById(customProfileId('1.12.2', 'forge'));
  assert.equal(custom.id, '1.12.2-forge');
  assert.equal(custom.custom, true);
  assert.equal(custom.name, 'Forge 1.12.2');
  assert.equal(buildCaption(custom), '1.12.2 · Forge');
});

test('unknown build falls back to the first one', () => {
  assert.equal(buildById('removed-profile').id, BUILD_IDS[0]);
});

test('versions compare numerically across the old and new numbering', () => {
  assert.ok(compareVersions('1.9', '1.21') < 0);
  assert.ok(compareVersions('1.21.11', '26.1') < 0);
  assert.equal(compareVersions('1.21', '1.21.0'), 0);
  assert.equal(hasLoader('1.16.5', 'fabric'), true);
  assert.equal(hasLoader('1.20.1', 'neoforge'), false);
  assert.equal(hasLoader('1.12.2', 'forge'), false);
});

test('modified versions appear only when the setting is on', () => {
  const releases = ['26.3', '1.20.1', '1.8.9'];

  assert.deepEqual(versionEntries(releases, false).map((entry) => entry.id), ['26.3-vanilla', '1.20.1-vanilla', '1.8.9-vanilla']);
  assert.deepEqual(versionEntries(releases, true).map((entry) => entry.title), [
    'Версия 26.3', 'Fabric 26.3 + Iris', 'Forge 26.3', 'NeoForge 26.3 + Iris',
    'Версия 1.20.1', 'Fabric 1.20.1 + Iris', 'Forge 1.20.1',
    'Версия 1.8.9',
  ]);
  assert.deepEqual(filterEntries(versionEntries(releases, true), 'fabric 1.20').map((entry) => entry.id), ['1.20.1-iris']);
});

test('default JVM arguments match the Rust core', () => {
  const config = readFileSync(new URL('../src-tauri/core/src/config.rs', import.meta.url), 'utf8');
  const rustDefault = config.match(/const DEFAULT_JVM_ARGS: &str = "([^"]+)";/)[1];

  assert.equal(DEFAULT_JVM_ARGS, rustDefault);
});

test('turned off arguments are shown as commented-out lines', () => {
  assert.equal(commentedArgs('-XX:+UseG1GC', '--fullscreen'), '// -XX:+UseG1GC\n// --fullscreen');
  assert.equal(commentedArgs('', ' '), '// аргументов нет');
});
