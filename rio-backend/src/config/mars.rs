use crate::config::colors::{ColorArray, ColorBuilder, ColorComposition, Colors, Format};
use crate::config::theme::AppearanceTheme;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Mars {
    #[serde(default)]
    pub appearance: Option<MarsAppearance>,
}

impl Mars {
    pub fn is_empty(&self) -> bool {
        self.appearance.is_none()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MarsAppearance {
    pub preset: MarsAppearancePreset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MarsAppearancePreset {
    Dark,
    Light,
    Auto,
}

impl MarsAppearancePreset {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            "auto" => Ok(Self::Auto),
            _ => Err(format!(
                "unknown Mars appearance preset `{raw}`; supported presets: dark, light, auto"
            )),
        }
    }

    pub fn forced_theme(self) -> Option<AppearanceTheme> {
        match self {
            Self::Dark => Some(AppearanceTheme::Dark),
            Self::Light => Some(AppearanceTheme::Light),
            Self::Auto => None,
        }
    }

    pub fn colors(self) -> Option<Colors> {
        match self {
            Self::Dark => Some(mars_dark_colors()),
            Self::Light => Some(mars_light_colors()),
            Self::Auto => None,
        }
    }
}

fn color(raw: &str) -> ColorArray {
    ColorBuilder::from_hex(raw.to_string(), Format::SRGB0_1)
        .expect("Mars appearance presets use valid opaque colors")
        .to_arr()
}

fn background(raw: &str) -> ColorComposition {
    let builder = ColorBuilder::from_hex(raw.to_string(), Format::SRGB0_1)
        .expect("Mars appearance presets use valid opaque colors");
    (builder.to_arr(), builder.to_wgpu())
}

fn mars_colors(background_color: &str, foreground: &str, dim_foreground: &str) -> Colors {
    Colors {
        background: background(background_color),
        foreground: color(foreground),
        dim_foreground: Some(color(dim_foreground)),
        black: color("#000000"),
        dim_black: Some(color("#6f7782")),
        red: color("#cd0000"),
        green: color("#00cd00"),
        yellow: color("#cdcd00"),
        blue: color("#1093f5"),
        magenta: color("#cd00cd"),
        cyan: color("#00cdcd"),
        white: color("#faebd7"),
        light_black: color("#8b949e"),
        light_red: color("#ff0000"),
        light_green: color("#00ff00"),
        light_yellow: color("#ffff00"),
        light_blue: color("#11b5f6"),
        light_magenta: color("#ff00ff"),
        light_cyan: color("#00ffff"),
        light_white: color("#ffffff"),
        ..Colors::default()
    }
}

pub fn mars_dark_colors() -> Colors {
    mars_colors("#111416", "#eeeeec", "#9d9d9c")
}

pub fn mars_light_colors() -> Colors {
    mars_colors("#f5f3ef", "#202124", "#62666d")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_appearance_presets() {
        let dark: Mars = toml::from_str(
            r##"
            [appearance]
            preset = "dark"
            "##,
        )
        .unwrap();
        let light: Mars = toml::from_str(
            r##"
            [appearance]
            preset = "light"
            "##,
        )
        .unwrap();

        assert_eq!(
            dark.appearance.unwrap().preset.forced_theme(),
            Some(AppearanceTheme::Dark)
        );
        assert_eq!(
            light.appearance.unwrap().preset.forced_theme(),
            Some(AppearanceTheme::Light)
        );
        assert_ne!(
            dark.appearance
                .unwrap()
                .preset
                .colors()
                .unwrap()
                .background
                .0,
            light
                .appearance
                .unwrap()
                .preset
                .colors()
                .unwrap()
                .background
                .0
        );
    }

    #[test]
    fn parses_auto_appearance_preset_without_forcing_theme() {
        let parsed: Mars = toml::from_str(
            r##"
            [appearance]
            preset = "auto"
            "##,
        )
        .unwrap();

        let preset = parsed.appearance.unwrap().preset;
        assert_eq!(preset.forced_theme(), None);
        assert!(preset.colors().is_none());
    }

    #[test]
    fn appearance_preset_rejects_unknown_name() {
        let err = toml::from_str::<Mars>(
            r##"
            [appearance]
            preset = "sepia"
            "##,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown variant `sepia`"));
    }
}
