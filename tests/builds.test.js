import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { BUILD_IDS, buildById, buildCaption, buildChips } from '../src/builds.js';

test('launcher builds match the profiles the Rust core accepts', () => {
  const config = readFileSync(new URL('../src-tauri/core/src/config.rs', import.meta.url), 'utf8');
  const block = config.match(/pub const PROFILES: \[&str; \d+\] = \[([\s\S]*?)\];/)[1];
  const rustProfiles = [...block.matchAll(/"([^"]+)"/g)].map((match) => match[1]);

  assert.deepEqual(BUILD_IDS, rustProfiles);
});

test('NeoForge 26.2 build runs shaders on NeoForge', () => {
  const build = buildById('26.2-iris-neoforge');

  assert.equal(build.version, '26.2');
  assert.deepEqual(buildChips(build), ['Minecraft 26.2', 'NeoForge', 'Шейдеры Iris']);
  assert.equal(buildCaption(build), '26.2 · NeoForge · Iris');
  assert.equal(buildCaption(buildById('26.3-vanilla')), '26.3 · Vanilla');
});

test('unknown build falls back to the first one', () => {
  assert.equal(buildById('removed-profile').id, BUILD_IDS[0]);
});
