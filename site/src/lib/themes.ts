export const THEMES = [
  'transit',
  'ink',
  'noir',
  'paper',
  'mono-light',
  'arctic',
  'slate',
  'nord',
  'sage',
  'storm',
  'midnight',
  'cobalt',
  'jade',
  'ember',
  'copper',
  'sepia',
] as const;

export type Theme = (typeof THEMES)[number];

export const THEME_LABELS: Record<Theme, string> = {
  transit: 'Transit',
  ink: 'Ink',
  noir: 'Noir',
  paper: 'Paper',
  'mono-light': 'Mono Light',
  arctic: 'Arctic',
  slate: 'Slate',
  nord: 'Nord',
  sage: 'Sage',
  storm: 'Storm',
  midnight: 'Midnight',
  cobalt: 'Cobalt',
  jade: 'Jade',
  ember: 'Ember',
  copper: 'Copper',
  sepia: 'Sepia',
};

export const THEME_DESCRIPTIONS: Record<Theme, string> = {
  transit: 'Deep navy/blue transit-map look (the classic default).',
  ink: 'Blueprint-style — white grid on deep blue, line-art nodes.',
  noir: 'High-contrast black & white — film noir aesthetic.',
  paper: 'Warm cream background with slate — print-friendly.',
  'mono-light': 'Clean light monochrome — print and presentation ready.',
  arctic: 'Bright, clean — white, pale blue, cool grey.',
  slate: 'Cool blue-grey — modern, understated.',
  nord: 'Nordic palette — frosty blues and muted snow.',
  sage: 'Muted green-grey — calm, organic, understated.',
  storm: 'Dark stormy grey with electric blue accents.',
  midnight: 'Ultra-dark indigo with cool silver accents.',
  cobalt: 'Rich cobalt blue with warm accents.',
  jade: 'Lush green gemstone with gold highlights.',
  ember: 'Warm amber/copper on dark.',
  copper: 'Warm copper and patina — arts-and-crafts palette.',
  sepia: 'Warm vintage photo tones — print-friendly sepia.',
};

export const DEFAULT_THEME: Theme = 'transit';