// One tag ships the pkm binary and this plugin with the same version: fail unless manifest.json and package.json
// carry the version of [workspace.package] in the root Cargo.toml.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const plugin = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const cargoPath = path.resolve(plugin, '..', '..', 'Cargo.toml');

export function workspaceVersion(toml) {
  const section = toml.match(/^\[workspace\.package\][^\S\n]*\n([\s\S]*?)(?=^\[|(?![\s\S]))/m);
  const version = section?.[1].match(/^version\s*=\s*"([^"]+)"/m);
  if (!version) throw new Error('no version under [workspace.package] in Cargo.toml');
  return version[1];
}

function main() {
  const want = workspaceVersion(fs.readFileSync(cargoPath, 'utf8'));
  const bad = [];
  for (const name of ['manifest.json', 'package.json']) {
    const got = JSON.parse(fs.readFileSync(path.join(plugin, name), 'utf8')).version;
    if (got !== want) bad.push(`${name}: ${got}`);
  }
  if (bad.length) {
    console.error(`version mismatch: Cargo.toml workspace is ${want}, ${bad.join(', ')}`);
    process.exit(1);
  }
  console.log(`version ${want} matches Cargo.toml`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
