import { describe, expect, it } from 'vitest';
import { activeTheme, GUARD_CLASS, THEME } from '../src/theme-guard';

describe('theme guard', () => {
  it('reads the theme from customCss first, then the appearance config', () => {
    expect(activeTheme({ customCss: { theme: 'Primary' } })).toBe('Primary');
    expect(activeTheme({ customCss: {}, vault: { getConfig: (k) => (k === 'cssTheme' ? 'Minimal' : null) } })).toBe('Minimal');
    expect(activeTheme({})).toBe('');
    expect(activeTheme({ vault: { getConfig: () => 42 } })).toBe('');
  });

  it('guards the snippet that ships in presets/b/theme', async () => {
    const fs = await import('node:fs');
    const url = new URL('../../../presets/b/theme/e11-theme.src.css', import.meta.url);
    const css = fs.readFileSync(url, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '').replace(/@font-face\s*\{[^}]*\}/g, '');
    const unguarded: string[] = [];
    for (const m of css.matchAll(/([^{}]+)\{/g))
      for (const s of (m[1] ?? '').split(/,(?![^(]*\))/).map((x) => x.trim()))
        if (s && s !== ':root' && !s.startsWith(`body.${GUARD_CLASS}`)) unguarded.push(s);
    expect(THEME).toBe('Primary');
    expect(unguarded).toEqual([]);
  });
});
