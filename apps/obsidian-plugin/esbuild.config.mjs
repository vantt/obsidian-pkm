// Bundles src/main.ts into dist/main.js and copies manifest.json and styles.css next to it: the three files
// Obsidian loads from .obsidian/plugins/pkm/. Templates (.md, .base) are bundled as text.
import { builtinModules } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import esbuild from 'esbuild';

const root = path.dirname(fileURLToPath(import.meta.url));
const dist = path.join(root, 'dist');
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(dist, { recursive: true });

await esbuild.build({
  entryPoints: [path.join(root, 'src', 'main.ts')],
  outfile: path.join(dist, 'main.js'),
  bundle: true,
  format: 'cjs',
  platform: 'node',
  target: 'es2022',
  // Provided by Obsidian at runtime (obsidian, CodeMirror, Electron) or by Node inside Electron.
  external: ['obsidian', 'electron', '@codemirror/*', '@lezer/*', ...builtinModules, ...builtinModules.map((m) => `node:${m}`)],
  loader: { '.md': 'text', '.base': 'text' },
  charset: 'utf8',
  legalComments: 'none',
  logLevel: 'info',
});

for (const name of ['manifest.json', 'styles.css']) fs.copyFileSync(path.join(root, name), path.join(dist, name));
console.log(`dist: ${fs.readdirSync(dist).join(', ')}`);
