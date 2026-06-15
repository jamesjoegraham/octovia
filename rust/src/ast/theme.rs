//! Theme system and SVG canvas background.
//!
//! Themes are loaded from the embedded `themes.json` manifest at compile
//! time. `Background` is intentionally separate from `ThemeColors` so the
//! outermost `<svg>` background can be controlled independently of label
//! halos and other opaque-on-canvas effects.

// ---------------------------------------------------------------------------
// Theme system — data-driven from themes.json
// ---------------------------------------------------------------------------

/// A single theme's colour palette.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemeColors {
    pub bg: String,
    pub node_fill: String,
    pub node_stroke: String,
    pub edge_forward: String,
    pub edge_cyclic: String,
    pub label_fill: String,
    pub title_fill: String,
    pub node_glow: String,
    pub gradient_start: String,
    pub gradient_end: String,
}

impl ThemeColors {
    /// Convenience accessor for backwards compatibility — returns self.
    pub fn colors(&self) -> &ThemeColors {
        self
    }

    /// Resolve a theme name to ThemeColors.
    pub fn from_str(name: &str) -> Option<ThemeColors> {
        resolve_theme(name)
    }
}

/// A theme entry from the themes.json file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemeEntry {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub name: String,
    pub description: String,
    pub bg: String,
    pub node_fill: String,
    pub node_stroke: String,
    pub edge_forward: String,
    pub edge_cyclic: String,
    pub label_fill: String,
    pub title_fill: String,
    pub node_glow: String,
    pub gradient_start: String,
    pub gradient_end: String,
}

/// The top-level themes.json structure.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ThemeManifest {
    pub themes: Vec<ThemeEntry>,
}

/// Load and parse the embedded themes.json at compile time.
pub fn load_builtin_themes() -> ThemeManifest {
    serde_json::from_str(include_str!("themes.json"))
        .expect("themes.json is invalid JSON or mismatched schema")
}

/// Resolve a theme name (or alias) to a `ThemeColors`, along with metadata.
/// Returns `None` if the name doesn't match any theme.
pub fn resolve_theme(name: &str) -> Option<ThemeColors> {
    let name_lower = name.trim().to_lowercase();
    let manifest = load_builtin_themes();
    manifest.themes.into_iter().find(|entry| {
        entry.id == name_lower
            || (entry.id.len() > name_lower.len() && entry.id[..name_lower.len()] == name_lower)
            || entry.aliases.iter().any(|a| *a == name_lower)
    }).map(|entry| ThemeColors {
        bg: entry.bg,
        node_fill: entry.node_fill,
        node_stroke: entry.node_stroke,
        edge_forward: entry.edge_forward,
        edge_cyclic: entry.edge_cyclic,
        label_fill: entry.label_fill,
        title_fill: entry.title_fill,
        node_glow: entry.node_glow,
        gradient_start: entry.gradient_start,
        gradient_end: entry.gradient_end,
    })
}

/// Get all built-in theme names and their display names.
pub fn list_themes() -> Vec<(String, String)> {
    let manifest = load_builtin_themes();
    manifest.themes.into_iter().map(|e| (e.id, e.name)).collect()
}

/// Get the default theme (Transit) colours.
pub fn default_theme() -> ThemeColors {
    resolve_theme("transit").expect("default theme 'transit' must exist in themes.json")
}

impl Default for ThemeColors {
    fn default() -> Self {
        default_theme()
    }
}

// ---------------------------------------------------------------------------
// Canvas background
// ---------------------------------------------------------------------------

/// Canvas background mode for the rendered SVG.
///
/// This is intentionally separate from the theme — the theme's `bg` is still
/// used for label halos and other opaque-on-canvas effects, while this enum
/// controls only the outermost `<svg style="background-color: ...">`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Background {
    /// No canvas fill — the SVG renders against whatever sits behind it.
    #[default]
    Transparent,
    /// Use the active theme's `bg` colour. Resolved at render time so a
    /// theme override applied after parsing still takes effect.
    Theme,
    /// An explicit CSS colour (hex, rgb(...), named colour, etc.).
    Custom(String),
}

impl Background {
    /// Parse a CLI / DSL value into a `Background`.
    ///
    /// - `"transparent"` (case-insensitive) → `Transparent`
    /// - `"theme"` (case-insensitive) → `Theme`
    /// - anything else → `Custom(value)`
    pub fn parse_value(value: &str) -> Self {
        let trimmed = value.trim();
        match trimmed.to_lowercase().as_str() {
            "transparent" => Background::Transparent,
            "theme" => Background::Theme,
            _ => Background::Custom(trimmed.to_string()),
        }
    }

    /// Resolve to the concrete CSS colour string used in the SVG output.
    pub fn resolve<'a>(&'a self, theme: &'a ThemeColors) -> &'a str {
        match self {
            Background::Transparent => "transparent",
            Background::Theme => &theme.bg,
            Background::Custom(s) => s.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_themes_non_empty() {
        let m = load_builtin_themes();
        assert!(!m.themes.is_empty(), "expected at least one theme");
    }

    #[test]
    fn test_resolve_transit() {
        let c = resolve_theme("transit").expect("transit");
        assert_eq!(c.bg, "#1A1A2E");
    }

    #[test]
    fn test_resolve_aliases() {
        assert!(resolve_theme("dark").is_some(), "alias 'dark' for transit");
        assert!(resolve_theme("cream").is_some(), "alias 'cream' for paper");
        assert!(resolve_theme("graphite").is_some(), "alias 'graphite' for mono-light");
    }

    #[test]
    fn test_resolve_unknown() {
        assert!(resolve_theme("nonexistent").is_none());
    }

    #[test]
    fn test_default_theme() {
        let c = default_theme();
        assert_eq!(c.bg, "#1A1A2E");
    }

    #[test]
    fn test_list_themes() {
        let list = list_themes();
        assert!(!list.is_empty());
        assert!(list.iter().any(|(id, _)| id == "transit"));
    }

    #[test]
    fn test_every_theme_has_valid_colors() {
        let m = load_builtin_themes();
        for entry in &m.themes {
            assert!(!entry.bg.is_empty(), "theme '{}' missing bg", entry.id);
            assert!(!entry.node_fill.is_empty(), "theme '{}' missing node_fill", entry.id);
            assert!(!entry.node_stroke.is_empty(), "theme '{}' missing node_stroke", entry.id);
            assert!(!entry.node_glow.is_empty(), "theme '{}' missing node_glow", entry.id);
            assert!(!entry.gradient_start.is_empty(), "theme '{}' missing gradient_start", entry.id);
        }
    }

    #[test]
    fn test_background_default_is_transparent() {
        let bg = Background::default();
        assert_eq!(bg, Background::Transparent);
        let theme = default_theme();
        assert_eq!(bg.resolve(&theme), "transparent");
    }

    #[test]
    fn test_background_theme_resolves_to_theme_bg() {
        let theme = default_theme();
        assert_eq!(Background::Theme.resolve(&theme), theme.bg.as_str());
    }

    #[test]
    fn test_background_custom_resolves_verbatim() {
        let theme = default_theme();
        let bg = Background::Custom("#abcdef".to_string());
        assert_eq!(bg.resolve(&theme), "#abcdef");
    }

    #[test]
    fn test_background_parse_value() {
        assert_eq!(Background::parse_value("transparent"), Background::Transparent);
        assert_eq!(Background::parse_value("  Transparent  "), Background::Transparent);
        assert_eq!(Background::parse_value("theme"), Background::Theme);
        assert_eq!(Background::parse_value("THEME"), Background::Theme);
        assert_eq!(
            Background::parse_value("#112233"),
            Background::Custom("#112233".to_string())
        );
    }
}
