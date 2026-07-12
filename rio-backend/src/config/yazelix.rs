use crate::config::colors::{ColorArray, ColorBuilder, Format};
use std::{path::Path, sync::OnceLock};
use yazelix_cursors::{
    CursorDefinition, CursorFamily, ResolvedCursorRegistryState, SplitDivider,
    SplitTransition,
};

pub const CURSOR_CONFIG_ENV: &str = "YAZELIX_CURSOR_CONFIG";
static CURSOR_STATE: OnceLock<Result<YazelixCursorState, String>> = OnceLock::new();

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Yazelix {
    pub cursor: Option<YazelixCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YazelixCursor {
    pub divider: YazelixCursorDivider,
    pub transition: YazelixCursorTransition,
    pub colors: [ColorArray; 2],
    pub cursor_color: ColorArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YazelixCursorDivider {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YazelixCursorTransition {
    Soft,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YazelixCursorState {
    pub cursor: Option<YazelixCursor>,
    pub trail_cursor: bool,
}

pub fn load_cursor_state(
    path: &Path,
    appearance: &str,
) -> Result<YazelixCursorState, String> {
    let registry =
        yazelix_cursors::load_cursor_config(path).map_err(|error| error.to_string())?;
    Ok(cursor_state(registry.resolve_for_appearance(appearance)))
}

pub fn load_cursor_state_once(
    path: &Path,
    appearance: &str,
) -> Result<YazelixCursorState, String> {
    CURSOR_STATE
        .get_or_init(|| load_cursor_state(path, appearance))
        .clone()
}

fn cursor_state(resolved: ResolvedCursorRegistryState) -> YazelixCursorState {
    YazelixCursorState {
        cursor: resolved
            .selected_cursor
            .as_ref()
            .map(cursor_from_definition),
        trail_cursor: !resolved.trail_disabled
            && resolved.selected_trail_effect.is_some(),
    }
}

fn cursor_from_definition(definition: &CursorDefinition) -> YazelixCursor {
    let primary = color(&definition.colors[0]);
    let secondary = match definition.family {
        CursorFamily::Mono => primary,
        CursorFamily::Split => color(&definition.colors[1]),
    };
    YazelixCursor {
        divider: match definition.divider.unwrap_or(SplitDivider::Vertical) {
            SplitDivider::Vertical => YazelixCursorDivider::Vertical,
            SplitDivider::Horizontal => YazelixCursorDivider::Horizontal,
        },
        transition: match definition.transition.unwrap_or(SplitTransition::Hard) {
            SplitTransition::Soft => YazelixCursorTransition::Soft,
            SplitTransition::Hard => YazelixCursorTransition::Hard,
        },
        colors: [primary, secondary],
        cursor_color: color(&definition.cursor_color),
    }
}

fn color(color: &yazelix_cursors::CursorColor) -> ColorArray {
    ColorBuilder::from_hex(color.hex.clone(), Format::SRGB0_1)
        .expect("yazelix-cursors validates cursor colors")
        .to_arr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::colors::hex_to_color_arr;
    use std::{fs, process};

    #[test]
    fn loads_child_owned_fixed_custom_random_and_none_cursor_states() {
        let path = std::env::temp_dir()
            .join(format!("mars-yazelix-cursors-{}.toml", process::id()));
        fs::write(
            &path,
            r##"
schema_version = 1
enabled_cursors = ["custom_split", "custom_mono", "midnight", "snow"]

[settings]
trail = "custom_split"
trail_effect = "tail"
mode_effect = "none"
glow = "none"
duration = 1.0
kitty_enable_cursor = true

[[cursor]]
name = "custom_split"
family = "split"
divider = "horizontal"
transition = "soft"
colors = ["#112233", "#445566"]
cursor_color = "#778899"

[[cursor]]
name = "custom_mono"
family = "mono"
color = "#abcdef"

[[cursor]]
name = "midnight"
family = "mono"
color = "#0f172a"

[[cursor]]
name = "snow"
family = "mono"
color = "#ffffff"
"##,
        )
        .unwrap();

        let registry = yazelix_cursors::load_cursor_config(&path).unwrap();
        let fixed = cursor_state(registry.resolve_with_entropy_for_appearance(0, "dark"));
        let split = fixed.cursor.unwrap();
        assert_eq!(split.divider, YazelixCursorDivider::Horizontal);
        assert_eq!(split.transition, YazelixCursorTransition::Soft);
        assert_eq!(split.colors[0], hex_to_color_arr("#112233"));
        assert_eq!(split.cursor_color, hex_to_color_arr("#778899"));
        assert!(fixed.trail_cursor);

        let mut mono = registry.clone();
        mono.settings.trail = "custom_mono".into();
        let mono = cursor_state(mono.resolve_with_entropy_for_appearance(0, "dark"));
        assert_eq!(
            mono.cursor.unwrap().colors,
            [hex_to_color_arr("#abcdef"); 2]
        );

        let mut random = registry.clone();
        random.enabled_cursors = vec!["midnight".into(), "snow".into()];
        random.settings.trail = "random".into();
        let random = cursor_state(random.resolve_with_entropy_for_appearance(0, "light"));
        assert_eq!(
            random.cursor.unwrap().colors,
            [hex_to_color_arr("#0f172a"); 2]
        );

        let mut none = registry;
        none.settings.trail = "none".into();
        let none = cursor_state(none.resolve_with_entropy_for_appearance(0, "dark"));
        assert_eq!(none.cursor, None);
        assert!(!none.trail_cursor);

        fs::write(&path, "schema_version = 2").unwrap();
        assert!(load_cursor_state(&path, "dark")
            .unwrap_err()
            .contains("Could not parse Yazelix cursor config"));
        fs::remove_file(path).unwrap();
    }
}
