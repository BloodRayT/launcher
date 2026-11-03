// Версия приложения хранится в трёх файлах. Расхождение ломает имена файлов релиза и автообновление.
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), 'utf8');

const versions = {
  'package.json': JSON.parse(read('package.json')).version,
  'src-tauri/tauri.conf.json': JSON.parse(read('src-tauri/tauri.conf.json')).version,
  'src-tauri/Cargo.toml': read('src-tauri/Cargo.toml').match(/^\[package\][\s\S]*?^version = "([^"]+)"/m)?.[1],
};

const tag = process.env.GITHUB_REF_TYPE === 'tag' ? process.env.GITHUB_REF_NAME : null;
if (tag) versions[`git tag ${tag}`] = tag.replace(/^v/, '');

const unique = new Set(Object.values(versions));
if (unique.size !== 1 || unique.has(undefined)) {
  console.error('Версии не совпадают:', versions);
  process.exit(1);
}
console.log(`Версия ${[...unique][0]} совпадает во всех файлах.`);
