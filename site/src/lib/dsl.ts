import { DEFAULT_THEME, type Theme } from './themes';

/** Read the `theme:` directive from a DSL document, defaulting to `DEFAULT_THEME`. */
export function currentTheme(dsl: string): Theme {
  for (const line of dsl.split('\n')) {
    const m = line.match(/^\s*theme\s*:\s*(\S+)/i);
    if (m) return m[1].toLowerCase() as Theme;
  }
  return DEFAULT_THEME;
}

/** Return a copy of `dsl` with its `theme:` directive set to `theme`, inserting one if missing. */
export function withTheme(dsl: string, theme: Theme): string {
  const lines = dsl.split('\n');
  const themeIdx = lines.findIndex((l) => /^\s*theme\s*:/i.test(l));
  if (themeIdx >= 0) {
    lines[themeIdx] = `theme: ${theme}`;
    return lines.join('\n');
  }
  // Insert directly after the last contiguous directive/comment line so the
  // new `theme:` line sits with the existing header block (no blank line gap).
  let insertAt = 0;
  for (let i = 0; i < lines.length; i++) {
    const t = lines[i].trim();
    if (t.startsWith('#') || /^\s*\w+\s*:/.test(t)) {
      insertAt = i + 1;
    } else {
      break;
    }
  }
  // If there is no header block at all, ensure a blank line after the directive.
  if (insertAt === 0) {
    lines.splice(0, 0, `theme: ${theme}`, '');
  } else {
    lines.splice(insertAt, 0, `theme: ${theme}`);
  }
  return lines.join('\n');
}
