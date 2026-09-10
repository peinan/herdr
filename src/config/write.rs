#[derive(Clone, Copy)]
pub(crate) enum ConfigEdit<'a> {
    Theme(&'a str),
    StatusIndicators(super::StatusIndicatorStyle),
    Sound(bool),
    ToastDelivery(super::ToastDelivery),
}

impl ConfigEdit<'_> {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Theme(_) => "theme",
            Self::StatusIndicators(_) => "status indicators",
            Self::Sound(_) => "sound setting",
            Self::ToastDelivery(_) => "toast setting",
        }
    }

    pub(crate) fn apply(self, content: &str) -> String {
        match self {
            Self::Theme(name) => {
                let content =
                    super::upsert_section_value(content, "theme", "name", &format!("\"{name}\""));
                super::upsert_section_bool(&content, "theme", "auto_switch", false)
            }
            Self::StatusIndicators(style) => super::upsert_section_value(
                content,
                "ui",
                "status_indicators",
                &format!("\"{}\"", style.as_str()),
            ),
            Self::Sound(enabled) => {
                super::upsert_section_bool(content, "ui.sound", "enabled", enabled)
            }
            Self::ToastDelivery(delivery) => {
                let value = match delivery {
                    super::ToastDelivery::Off => "\"off\"",
                    super::ToastDelivery::Herdr => "\"herdr\"",
                    super::ToastDelivery::Terminal => "\"terminal\"",
                    super::ToastDelivery::System => "\"system\"",
                };
                let content = super::upsert_section_value(content, "ui.toast", "delivery", value);
                super::remove_section_key(&content, "ui.toast", "enabled")
            }
        }
    }
}

pub(crate) fn update_file_at(
    path: &std::path::Path,
    description: &str,
    update: impl FnOnce(&str) -> String,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create config directory: {error}"))?;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "failed to read config before saving {description}: {error}"
            ));
        }
    };
    std::fs::write(path, update(&content))
        .map_err(|error| format!("failed to save {description}: {error}"))
}

pub(crate) fn write_edit(edit: ConfigEdit<'_>) -> Result<(), String> {
    update_file_at(&super::config_path(), edit.description(), |content| {
        edit.apply(content)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings edits rewrite config.toml textually. A `[keys]` section that uses
    /// the fork's table-form bindings (`{ key, repeat }`) and `repeat_timeout`
    /// must survive every edit untouched and still parse afterwards.
    #[test]
    fn config_edits_preserve_table_form_keybindings() {
        let original = concat!(
            "[keys]\n",
            "repeat_timeout = 750\n",
            "swap_pane_down = { key = \"prefix+shift+j\", repeat = true }\n",
            "next_tab = [\"prefix+n\", \"prefix+l\"]\n",
            "\n",
            "[ui]\n",
            "pane_borders = \"always\"\n",
        );
        let edits = [
            ConfigEdit::Theme("mocha"),
            ConfigEdit::StatusIndicators(super::super::StatusIndicatorStyle::Symbols),
            ConfigEdit::Sound(false),
            ConfigEdit::ToastDelivery(super::super::ToastDelivery::Off),
        ];

        for edit in edits {
            let edited = edit.apply(original);
            assert!(
                edited.contains("swap_pane_down = { key = \"prefix+shift+j\", repeat = true }"),
                "{} edit rewrote the table binding:\n{edited}",
                edit.description()
            );
            let config: super::super::Config = toml::from_str(&edited).unwrap_or_else(|err| {
                panic!("{} edit broke the config: {err}", edit.description())
            });
            assert_eq!(config.keys.repeat_timeout, 750);
            let keybinds = config.keybinds();
            assert_eq!(keybinds.swap_pane_down.bindings.len(), 1);
            assert!(keybinds.swap_pane_down.bindings[0].repeatable);
            assert_eq!(keybinds.next_tab.bindings.len(), 2);
        }
    }
}
