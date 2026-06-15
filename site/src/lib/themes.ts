export const THEMES = ['transit', 'ember', 'forest', 'light', 'monochrome'] as const;

export type Theme = (typeof THEMES)[number];

export const THEME_LABELS: Record<Theme, string> = {
  transit: 'Transit',
  ember: 'Ember',
  forest: 'Forest',
  light: 'Light',
  monochrome: 'Mono',
};

export const DEFAULT_THEME: Theme = 'monochrome';
