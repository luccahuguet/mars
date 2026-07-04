use crate::config::colors::{ColorArray, ColorBuilder, Format};
use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Yazelix {
    #[serde(default)]
    pub cursor: Option<YazelixCursor>,
}

impl Yazelix {
    pub fn is_empty(&self) -> bool {
        self.cursor.is_none()
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct YazelixCursor {
    pub family: YazelixCursorFamily,
    pub divider: YazelixCursorDivider,
    pub transition: YazelixCursorTransition,
    pub colors: [ColorArray; 2],
    /// Yazelix registry cursor color. Split sprite rendering uses `colors`.
    pub cursor_color: Option<ColorArray>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum YazelixCursorFamily {
    Split,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum YazelixCursorDivider {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum YazelixCursorTransition {
    Hard,
    Soft,
}

#[derive(Debug, Clone, Copy)]
struct YazelixCursorPreset {
    name: &'static str,
    kind: YazelixCursorPresetKind,
}

#[derive(Debug, Clone, Copy)]
enum YazelixCursorPresetKind {
    Mono {
        color: &'static str,
    },
    Split {
        divider: YazelixCursorDivider,
        transition: YazelixCursorTransition,
        primary: &'static str,
        secondary: &'static str,
        cursor_color: &'static str,
    },
}

/// Mirrors the enabled cursor definitions in Yazelix's `yazelix_cursors_default.toml`.
const YAZELIX_CURSOR_PRESETS: &[YazelixCursorPreset] = &[
    YazelixCursorPreset {
        name: "blaze",
        kind: YazelixCursorPresetKind::Mono { color: "#ffb929" },
    },
    YazelixCursorPreset {
        name: "snow",
        kind: YazelixCursorPresetKind::Mono { color: "#ffffff" },
    },
    YazelixCursorPreset {
        name: "ice",
        kind: YazelixCursorPresetKind::Mono { color: "#38bdf8" },
    },
    YazelixCursorPreset {
        name: "midnight",
        kind: YazelixCursorPresetKind::Mono { color: "#0f172a" },
    },
    YazelixCursorPreset {
        name: "cosmic",
        kind: YazelixCursorPresetKind::Mono { color: "#c761f5" },
    },
    YazelixCursorPreset {
        name: "ocean",
        kind: YazelixCursorPresetKind::Mono { color: "#5ea8ff" },
    },
    YazelixCursorPreset {
        name: "forest",
        kind: YazelixCursorPresetKind::Mono { color: "#3bd17a" },
    },
    YazelixCursorPreset {
        name: "sunset",
        kind: YazelixCursorPresetKind::Mono { color: "#ff7a59" },
    },
    YazelixCursorPreset {
        name: "eclipse",
        kind: YazelixCursorPresetKind::Split {
            divider: YazelixCursorDivider::Vertical,
            transition: YazelixCursorTransition::Soft,
            primary: "#2e294e",
            secondary: "#ffd400",
            cursor_color: "#ffd400",
        },
    },
    YazelixCursorPreset {
        name: "dusk",
        kind: YazelixCursorPresetKind::Split {
            divider: YazelixCursorDivider::Vertical,
            transition: YazelixCursorTransition::Soft,
            primary: "#1e1e2f",
            secondary: "#e94560",
            cursor_color: "#e94560",
        },
    },
    YazelixCursorPreset {
        name: "orchid",
        kind: YazelixCursorPresetKind::Split {
            divider: YazelixCursorDivider::Vertical,
            transition: YazelixCursorTransition::Hard,
            primary: "#ff6b00",
            secondary: "#206dce",
            cursor_color: "#ff6b00",
        },
    },
    YazelixCursorPreset {
        name: "reef",
        kind: YazelixCursorPresetKind::Split {
            divider: YazelixCursorDivider::Vertical,
            transition: YazelixCursorTransition::Soft,
            primary: "#00e6ff",
            secondary: "#00ff66",
            cursor_color: "#00e6ff",
        },
    },
    YazelixCursorPreset {
        name: "magma",
        kind: YazelixCursorPresetKind::Split {
            divider: YazelixCursorDivider::Horizontal,
            transition: YazelixCursorTransition::Soft,
            primary: "#ff1600",
            secondary: "#2a3340",
            cursor_color: "#ff1600",
        },
    },
];

pub fn yazelix_cursor_preset_names() -> impl Iterator<Item = &'static str> {
    YAZELIX_CURSOR_PRESETS.iter().map(|preset| preset.name)
}

impl<'de> Deserialize<'de> for YazelixCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case", deny_unknown_fields)]
        struct RawYazelixCursor {
            #[serde(default)]
            preset: Option<String>,
            #[serde(default)]
            family: Option<YazelixCursorFamily>,
            #[serde(default)]
            divider: Option<YazelixCursorDivider>,
            #[serde(default)]
            transition: Option<String>,
            #[serde(default)]
            colors: Option<Vec<String>>,
            #[serde(default, rename = "cursor_color", alias = "cursor-color")]
            cursor_color: Option<String>,
        }

        let raw = RawYazelixCursor::deserialize(deserializer)?;
        if let Some(preset) = raw.preset {
            if raw.family.is_some()
                || raw.divider.is_some()
                || raw.transition.is_some()
                || raw.colors.is_some()
                || raw.cursor_color.is_some()
            {
                return Err(de::Error::custom(
                    "yazelix.cursor preset cannot be combined with manual cursor fields",
                ));
            }
            return resolve_preset(&preset).map_err(de::Error::custom);
        }

        let colors = raw
            .colors
            .ok_or_else(|| de::Error::missing_field("colors"))?;
        let colors = match colors.as_slice() {
            [primary, secondary] => [
                parse_color(primary).map_err(de::Error::custom)?,
                parse_color(secondary).map_err(de::Error::custom)?,
            ],
            _ => {
                return Err(de::Error::custom(
                    "yazelix.cursor split cursors require exactly two colors",
                ));
            }
        };
        let cursor_color = raw
            .cursor_color
            .as_deref()
            .map(parse_color)
            .transpose()
            .map_err(de::Error::custom)?;

        Ok(Self {
            family: raw
                .family
                .ok_or_else(|| de::Error::missing_field("family"))?,
            divider: raw
                .divider
                .ok_or_else(|| de::Error::missing_field("divider"))?,
            transition: parse_manual_transition(
                &raw.transition
                    .ok_or_else(|| de::Error::missing_field("transition"))?,
            )
            .map_err(de::Error::custom)?,
            colors,
            cursor_color,
        })
    }
}

fn parse_manual_transition(raw: &str) -> Result<YazelixCursorTransition, String> {
    match raw {
        "hard" => Ok(YazelixCursorTransition::Hard),
        other => Err(format!("unknown variant `{other}`, expected `hard`")),
    }
}

fn resolve_preset(raw: &str) -> Result<YazelixCursor, String> {
    let preset = raw.trim().to_ascii_lowercase();
    let Some(preset) = YAZELIX_CURSOR_PRESETS
        .iter()
        .find(|candidate| candidate.name == preset)
    else {
        return Err(format!(
            "unknown yazelix.cursor preset `{raw}`; supported presets: {}",
            yazelix_cursor_preset_names().collect::<Vec<_>>().join(", ")
        ));
    };

    match preset.kind {
        YazelixCursorPresetKind::Mono { color } => mono_preset(color),
        YazelixCursorPresetKind::Split {
            divider,
            transition,
            primary,
            secondary,
            cursor_color,
        } => split_preset(divider, transition, primary, secondary, cursor_color),
    }
}

fn mono_preset(color: &str) -> Result<YazelixCursor, String> {
    let color = parse_color(color)?;
    Ok(YazelixCursor {
        family: YazelixCursorFamily::Split,
        divider: YazelixCursorDivider::Vertical,
        transition: YazelixCursorTransition::Hard,
        colors: [color, color],
        cursor_color: Some(color),
    })
}

fn split_preset(
    divider: YazelixCursorDivider,
    transition: YazelixCursorTransition,
    primary: &str,
    secondary: &str,
    cursor_color: &str,
) -> Result<YazelixCursor, String> {
    Ok(YazelixCursor {
        family: YazelixCursorFamily::Split,
        divider,
        transition,
        colors: [parse_color(primary)?, parse_color(secondary)?],
        cursor_color: Some(parse_color(cursor_color)?),
    })
}

fn parse_color(raw: &str) -> Result<ColorArray, String> {
    let Some(hex) = raw.strip_prefix('#') else {
        return Err(String::from(
            "yazelix.cursor colors must be opaque #RRGGBB values",
        ));
    };
    if hex.len() != 6 {
        return Err(String::from(
            "yazelix.cursor colors must be opaque #RRGGBB values",
        ));
    }
    ColorBuilder::from_hex(raw.to_string(), Format::SRGB0_1).map(|color| color.to_arr())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::colors::hex_to_color_arr;

    #[test]
    fn cursor_preset_names_match_yazelix_registry() {
        assert_eq!(
            yazelix_cursor_preset_names().collect::<Vec<_>>(),
            vec![
                "blaze", "snow", "ice", "midnight", "cosmic", "ocean", "forest",
                "sunset", "eclipse", "dusk", "orchid", "reef", "magma"
            ]
        );
    }

    #[test]
    fn parses_split_cursor_options() {
        let parsed: Yazelix = toml::from_str(
            r##"
            [cursor]
            family = "split"
            divider = "vertical"
            transition = "hard"
            colors = ["#00e6ff", "#00ff66"]
            cursor_color = "#00e6ff"
            "##,
        )
        .unwrap();

        let cursor = parsed.cursor.unwrap();
        assert_eq!(cursor.family, YazelixCursorFamily::Split);
        assert_eq!(cursor.divider, YazelixCursorDivider::Vertical);
        assert_eq!(cursor.transition, YazelixCursorTransition::Hard);
        assert_eq!(cursor.colors[0], hex_to_color_arr("#00e6ff"));
        assert_eq!(cursor.colors[1], hex_to_color_arr("#00ff66"));
        assert_eq!(cursor.cursor_color, Some(hex_to_color_arr("#00e6ff")));
    }

    #[test]
    fn parses_reef_cursor_preset_from_yazelix_registry() {
        let parsed: Yazelix = toml::from_str(
            r##"
            [cursor]
            preset = "reef"
            "##,
        )
        .unwrap();

        let cursor = parsed.cursor.unwrap();
        assert_eq!(cursor.family, YazelixCursorFamily::Split);
        assert_eq!(cursor.divider, YazelixCursorDivider::Vertical);
        assert_eq!(cursor.transition, YazelixCursorTransition::Soft);
        assert_eq!(cursor.colors[0], hex_to_color_arr("#00e6ff"));
        assert_eq!(cursor.colors[1], hex_to_color_arr("#00ff66"));
        assert_eq!(cursor.cursor_color, Some(hex_to_color_arr("#00e6ff")));
    }

    #[test]
    fn parses_magma_cursor_preset_from_yazelix_registry() {
        let parsed: Yazelix = toml::from_str(
            r##"
            [cursor]
            preset = "magma"
            "##,
        )
        .unwrap();

        let cursor = parsed.cursor.unwrap();
        assert_eq!(cursor.family, YazelixCursorFamily::Split);
        assert_eq!(cursor.divider, YazelixCursorDivider::Horizontal);
        assert_eq!(cursor.transition, YazelixCursorTransition::Soft);
        assert_eq!(cursor.colors[0], hex_to_color_arr("#ff1600"));
        assert_eq!(cursor.colors[1], hex_to_color_arr("#2a3340"));
        assert_eq!(cursor.cursor_color, Some(hex_to_color_arr("#ff1600")));
    }

    #[test]
    fn projects_mono_cursor_presets_to_single_color_split_cursor() {
        let parsed: Yazelix = toml::from_str(
            r##"
            [cursor]
            preset = "snow"
            "##,
        )
        .unwrap();

        let cursor = parsed.cursor.unwrap();
        assert_eq!(cursor.family, YazelixCursorFamily::Split);
        assert_eq!(cursor.divider, YazelixCursorDivider::Vertical);
        assert_eq!(cursor.transition, YazelixCursorTransition::Hard);
        assert_eq!(cursor.colors[0], hex_to_color_arr("#ffffff"));
        assert_eq!(cursor.colors[1], hex_to_color_arr("#ffffff"));
        assert_eq!(cursor.cursor_color, Some(hex_to_color_arr("#ffffff")));
    }

    #[test]
    fn parses_sunset_cursor_preset_from_yazelix_registry() {
        let parsed: Yazelix = toml::from_str(
            r##"
            [cursor]
            preset = "sunset"
            "##,
        )
        .unwrap();

        let cursor = parsed.cursor.unwrap();
        assert_eq!(cursor.colors[0], hex_to_color_arr("#ff7a59"));
        assert_eq!(cursor.colors[1], hex_to_color_arr("#ff7a59"));
        assert_eq!(cursor.cursor_color, Some(hex_to_color_arr("#ff7a59")));
    }

    #[test]
    fn cursor_preset_rejects_unknown_name() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            preset = "plasma"
            "##,
        )
        .unwrap_err();

        let err = err.to_string();
        assert!(err.contains("unknown yazelix.cursor preset `plasma`"));
        assert!(err.contains("reef"));
        assert!(err.contains("magma"));
    }

    #[test]
    fn cursor_preset_rejects_manual_field_mix() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            preset = "reef"
            colors = ["#00e6ff", "#00ff66"]
            "##,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("preset cannot be combined with manual cursor fields"));
    }

    #[test]
    fn split_cursor_requires_exactly_two_colors() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "hard"
            colors = ["#ff1600"]
            "##,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("split cursors require exactly two colors"));
    }

    #[test]
    fn split_cursor_accepts_ghostty_style_cursor_color_alias() {
        let parsed: Yazelix = toml::from_str(
            r##"
            [cursor]
            family = "split"
            divider = "vertical"
            transition = "hard"
            colors = ["#00e6ff", "#00ff66"]
            cursor-color = "#00e6ff"
            "##,
        )
        .unwrap();

        let cursor = parsed.cursor.unwrap();
        assert_eq!(cursor.cursor_color, Some(hex_to_color_arr("#00e6ff")));
    }

    #[test]
    fn split_cursor_rejects_unknown_fields() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "hard"
            colors = ["#ff1600", "#20242f"]
            effect = "tail"
            "##,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn split_cursor_rejects_unknown_yazelix_fields() {
        let err = toml::from_str::<Yazelix>(
            r##"
            mode = "surprise"

            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "hard"
            colors = ["#ff1600", "#20242f"]
            "##,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn split_cursor_requires_transition() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            colors = ["#ff1600", "#20242f"]
            "##,
        )
        .unwrap_err();

        assert!(err.to_string().contains("missing field `transition`"));
    }

    #[test]
    fn split_cursor_rejects_soft_transition_until_supported() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "soft"
            colors = ["#ff1600", "#20242f"]
            "##,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown variant `soft`"));
    }

    #[test]
    fn split_cursor_rejects_alpha_colors() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "hard"
            colors = ["#ff160080", "#20242f"]
            "##,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("colors must be opaque #RRGGBB values"));
    }

    #[test]
    fn split_cursor_rejects_colors_without_hash_prefix() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "hard"
            colors = ["ff1600", "#20242f"]
            "##,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("colors must be opaque #RRGGBB values"));
    }

    #[test]
    fn split_cursor_rejects_padded_color_strings() {
        let err = toml::from_str::<Yazelix>(
            r##"
            [cursor]
            family = "split"
            divider = "horizontal"
            transition = "hard"
            colors = [" #ff1600", "#20242f"]
            "##,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("colors must be opaque #RRGGBB values"));
    }
}
