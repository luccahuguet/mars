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
}

impl<'de> Deserialize<'de> for YazelixCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case", deny_unknown_fields)]
        struct RawYazelixCursor {
            family: YazelixCursorFamily,
            divider: YazelixCursorDivider,
            transition: YazelixCursorTransition,
            colors: Vec<String>,
            #[serde(default, rename = "cursor_color", alias = "cursor-color")]
            cursor_color: Option<String>,
        }

        let raw = RawYazelixCursor::deserialize(deserializer)?;
        let colors = match raw.colors.as_slice() {
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
            family: raw.family,
            divider: raw.divider,
            transition: raw.transition,
            colors,
            cursor_color,
        })
    }
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
