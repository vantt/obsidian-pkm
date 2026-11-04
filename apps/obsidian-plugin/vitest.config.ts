import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [
    {
      name: 'text-templates',
      enforce: 'pre',
      transform(code, id) {
        if (/\.(md|base)$/.test(id)) return { code: `export default ${JSON.stringify(code)};`, map: null };
        return null;
      },
    },
  ],
  resolve: {
    // The obsidian package ships types only; tests run against a small stand-in.
    alias: { obsidian: fileURLToPath(new URL('./test/obsidian-mock.ts', import.meta.url)) },
  },
  test: { include: ['test/**/*.test.ts'] },
});
