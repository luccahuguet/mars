use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bell {
    #[serde(default = "default_audio_bell")]
    pub audio: bool,
    #[serde(default = "bool::default")]
    pub visual: bool,
}

impl Default for Bell {
    fn default() -> Self {
        Bell {
            audio: default_audio_bell(),
            visual: false,
        }
    }
}

fn default_audio_bell() -> bool {
    // Enable audio bell by default on macOS and Windows since they use the system sound
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        true
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Bell;
    use crate::config::Config;

    #[test]
    fn visual_bell_defaults_to_off() {
        assert!(!Bell::default().visual);
    }

    #[test]
    fn visual_only_bell_deserializes() {
        let config: Config = toml::from_str(
            r#"
            [bell]
            audio = false
            visual = true
            "#,
        )
        .unwrap();

        assert!(!config.bell.audio);
        assert!(config.bell.visual);
    }

    #[test]
    fn bell_can_be_disabled() {
        let config: Config = toml::from_str(
            r#"
            [bell]
            audio = false
            visual = false
            "#,
        )
        .unwrap();

        assert!(!config.bell.audio);
        assert!(!config.bell.visual);
    }
}
