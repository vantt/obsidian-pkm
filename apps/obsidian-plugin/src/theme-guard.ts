// The e11-theme snippet (presets/b/theme) is written for the Primary theme: its variables, its bare "r,g,b" callout
// colours. Obsidian applies an enabled snippet under every theme, so on another theme it would repaint that theme
// with Primary's fixes and break its callouts. While the active theme is THEME this puts GUARD_CLASS on <body>;
// every snippet rule starts with body.GUARD_CLASS, so under another theme (or with this plugin off) the snippet is
// silent. Same contract as scripts/e11/theme-guard-plugin in obsidian-pkm-automation.
import type { App, Plugin } from 'obsidian';

/** Theme folder name in .obsidian/themes, as Settings -> Appearance shows it. */
export const THEME = 'Primary';
/** Must match the prefix of every rule in presets/b/theme/e11-theme.src.css. */
export const GUARD_CLASS = 'e11-on-primary';

interface ThemeApp {
  customCss?: { theme?: string };
  vault?: { getConfig?: (key: string) => unknown };
}

/** Active theme name. `app.customCss.theme` is not public API; the appearance config is the fallback. */
export function activeTheme(app: App | ThemeApp): string {
  const a = app as ThemeApp;
  const theme = a.customCss?.theme ?? a.vault?.getConfig?.('cssTheme');
  return typeof theme === 'string' ? theme : '';
}

export function startThemeGuard(plugin: Plugin, body: HTMLElement = document.body): void {
  const sync = () => body.classList.toggle(GUARD_CLASS, activeTheme(plugin.app) === THEME);
  sync();
  // css-change fires when the theme or a snippet changes (Settings -> Appearance)
  plugin.registerEvent(plugin.app.workspace.on('css-change', sync));
  plugin.app.workspace.onLayoutReady(sync);
  plugin.register(() => body.classList.remove(GUARD_CLASS));
}
