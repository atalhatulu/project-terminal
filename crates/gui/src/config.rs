use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FontConfig {
    pub size: Option<f32>,
    pub family: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColorsConfig {
    pub background: Option<String>,
    pub foreground: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorConfig {
    pub color: Option<String>,
    pub style: Option<String>,
    pub blink: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowConfig {
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindingsConfig {
    pub new_tab: Option<String>,
    pub close_tab: Option<String>,
    pub next_tab: Option<String>,
    pub prev_tab: Option<String>,
    pub search: Option<String>,
    pub copy: Option<String>,
    pub paste: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EffectsConfig {
    pub typewriter: Option<bool>,
    pub typewriter_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub theme: Option<String>,
    pub font: Option<FontConfig>,
    pub colors: Option<ColorsConfig>,
    pub cursor: Option<CursorConfig>,
    pub window: Option<WindowConfig>,
    pub keybindings: Option<KeybindingsConfig>,
    pub shell: Option<String>,
    pub scrollback_lines: Option<usize>,
    pub effects: Option<EffectsConfig>,
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config/project-terminal/config.toml"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(cfg) = toml::from_str::<Config>(&content) {
                        return cfg;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn font_size(&self) -> f32 {
        self.font.as_ref().and_then(|f| f.size).unwrap_or(14.0)
    }

    pub fn font_family(&self) -> Option<String> {
        self.font
            .as_ref()
            .and_then(|f| f.family.clone())
            .filter(|fam| fam != "monospace")
    }

    pub fn cursor_shape(&self) -> Option<term_core::CursorShape> {
        match self.cursor.as_ref().and_then(|c| c.style.as_deref()) {
            Some("beam") | Some("bar") => Some(term_core::CursorShape::Beam),
            Some("underline") => Some(term_core::CursorShape::Underline),
            Some("block") => Some(term_core::CursorShape::Block),
            _ => None,
        }
    }

    pub fn resolved_theme(&self) -> Option<&'static crate::theme::Theme> {
        self.theme.as_deref().and_then(crate::theme::get_theme)
    }

    pub fn background_color(&self) -> (u8, u8, u8) {
        if let Some(bg) = self
            .colors
            .as_ref()
            .and_then(|c| c.background.as_deref())
            .and_then(parse_hex_color)
        {
            return bg;
        }
        self.resolved_theme()
            .map(|t| t.background)
            .unwrap_or(crate::theme::PURE_BLACK.background)
    }

    pub fn foreground_color(&self) -> (u8, u8, u8) {
        if let Some(fg) = self
            .colors
            .as_ref()
            .and_then(|c| c.foreground.as_deref())
            .and_then(parse_hex_color)
        {
            return fg;
        }
        self.resolved_theme()
            .map(|t| t.foreground)
            .unwrap_or(crate::theme::PURE_BLACK.foreground)
    }

    pub fn cursor_color(&self) -> [f32; 4] {
        if let Some((r, g, b)) = self
            .cursor
            .as_ref()
            .and_then(|c| c.color.as_deref())
            .and_then(parse_hex_color)
        {
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 0.9]
        } else if let Some(theme) = self.resolved_theme() {
            theme.cursor
        } else {
            crate::theme::PURE_BLACK.cursor
        }
    }

    pub fn palette(&self) -> [(u8, u8, u8); 16] {
        self.resolved_theme()
            .map(|t| t.palette)
            .unwrap_or(crate::theme::PURE_BLACK.palette)
    }

    pub fn opacity(&self) -> f32 {
        self.window
            .as_ref()
            .and_then(|w| w.opacity)
            .unwrap_or(1.0)
            .clamp(0.05, 1.0)
    }
}

fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    } else {
        None
    }
}
