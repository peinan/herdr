use std::borrow::Cow;

use ratatui::layout::Rect;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::config::PanePadding;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopupSize {
    Cells(u16),
    Percent(u8),
}

impl PopupSize {
    pub(crate) fn resolve(self, available: u16) -> u16 {
        match self {
            Self::Cells(cells) => cells,
            Self::Percent(percent) => ((available as u32 * percent as u32) / 100) as u16,
        }
    }

    pub(crate) fn parse_cli(value: &str) -> Result<Self, String> {
        if let Some(percent) = value.strip_suffix('%') {
            let percent = percent
                .parse::<u8>()
                .map_err(|_| "must be a number of cells or a percentage like 80%".to_string())?;
            if !(1..=100).contains(&percent) {
                return Err("percentage must be between 1% and 100%".to_string());
            }
            return Ok(Self::Percent(percent));
        }
        value
            .parse::<u16>()
            .map(Self::Cells)
            .map_err(|_| "must be a number of cells or a percentage like 80%".to_string())
    }

    fn parse_percent_string(value: &str) -> Result<Self, String> {
        if value.ends_with('%') {
            return Self::parse_cli(value);
        }
        Err("string sizes must be percentages like 80%; use a number for cells".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PopupResolvedGeometry {
    pub outer: Rect,
    pub inner: Rect,
}

/// Resolves the popup's outer frame and the terminal area inside it.
///
/// `padding` (`[ui] popup_padding`) is applied inside the border and outside
/// the reserved spacer column, mirroring how a normal pane composes its inner
/// rect. Both the server, which sizes the popup PTY, and the client, which
/// blits the rendered frame, resolve the geometry through this function, so
/// they must pass the same padding to stay in agreement.
pub(crate) fn resolve_popup_geometry(
    width: Option<PopupSize>,
    height: Option<PopupSize>,
    padding: PanePadding,
    area: Rect,
) -> Option<PopupResolvedGeometry> {
    let default_width = area.width.saturating_div(2).max(6);
    let default_height = area.height.saturating_div(2).max(4);
    let outer_width = width
        .map(|width| width.resolve(area.width))
        .unwrap_or(default_width)
        .max(6)
        .min(area.width);
    let outer_height = height
        .map(|height| height.resolve(area.height))
        .unwrap_or(default_height)
        .max(4)
        .min(area.height);
    if outer_width < 6 || outer_height < 4 {
        return None;
    }

    let outer_x = area.x + (area.width.saturating_sub(outer_width)) / 2;
    let outer_y = area.y + (area.height.saturating_sub(outer_height)) / 2;
    let pane_inner = crate::ui::apply_pane_padding(
        Rect::new(
            outer_x.saturating_add(1),
            outer_y.saturating_add(1),
            outer_width.saturating_sub(2),
            outer_height.saturating_sub(2),
        ),
        padding,
    );
    let terminal_cols = if pane_inner.width <= 4 {
        pane_inner.width
    } else {
        pane_inner.width.saturating_sub(1)
    };
    let inner = Rect::new(pane_inner.x, pane_inner.y, terminal_cols, pane_inner.height);
    Some(PopupResolvedGeometry {
        outer: Rect::new(outer_x, outer_y, outer_width, outer_height),
        inner,
    })
}

impl Serialize for PopupSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Cells(cells) => serializer.serialize_u16(*cells),
            Self::Percent(percent) => serializer.serialize_str(&format!("{percent}%")),
        }
    }
}

impl<'de> Deserialize<'de> for PopupSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PopupSizeVisitor;

        impl serde::de::Visitor<'_> for PopupSizeVisitor {
            type Value = PopupSize;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a cell count or percentage string like 80%")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let value =
                    u16::try_from(value).map_err(|_| E::custom("cell count must fit in u16"))?;
                Ok(PopupSize::Cells(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let value = u16::try_from(value)
                    .map_err(|_| E::custom("cell count must be between 0 and 65535"))?;
                Ok(PopupSize::Cells(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                PopupSize::parse_percent_string(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(PopupSizeVisitor)
    }
}

impl schemars::JsonSchema for PopupSize {
    fn schema_name() -> Cow<'static, str> {
        "PopupSize".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "oneOf": [
                {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 65535,
                    "description": "Outer popup size in terminal cells, including the border."
                },
                {
                    "type": "string",
                    "pattern": "^(100|[1-9][0-9]?)%$",
                    "description": "Outer popup size as a percentage of the terminal area, for example 80%."
                }
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PanePadding, PopupSize};

    #[test]
    fn parses_cells_and_percent() {
        assert_eq!(PopupSize::parse_cli("120"), Ok(PopupSize::Cells(120)));
        assert_eq!(PopupSize::parse_cli("80%"), Ok(PopupSize::Percent(80)));
        assert_eq!(PopupSize::Percent(80).resolve(100), 80);
    }

    #[test]
    fn rejects_invalid_percent() {
        assert!(PopupSize::parse_cli("0%").is_err());
        assert!(PopupSize::parse_cli("101%").is_err());
        assert!(PopupSize::parse_cli("%").is_err());
    }

    #[test]
    fn string_deserialization_requires_percent() {
        assert!(serde_json::from_value::<PopupSize>(serde_json::json!("120")).is_err());
        assert_eq!(
            serde_json::from_value::<PopupSize>(serde_json::json!("80%")).unwrap(),
            PopupSize::Percent(80)
        );
    }

    #[test]
    fn serializes_percent_as_string() {
        assert_eq!(
            serde_json::to_value(PopupSize::Percent(80)).unwrap(),
            serde_json::json!("80%")
        );
        assert_eq!(
            serde_json::to_value(PopupSize::Cells(120)).unwrap(),
            serde_json::json!(120)
        );
    }

    #[test]
    fn resolves_requested_outer_size_and_inner_terminal_area() {
        let resolved = super::resolve_popup_geometry(
            Some(PopupSize::Percent(80)),
            Some(PopupSize::Percent(40)),
            PanePadding::default(),
            ratatui::layout::Rect::new(0, 0, 100, 30),
        )
        .unwrap();
        assert_eq!(resolved.outer, ratatui::layout::Rect::new(10, 9, 80, 12));
        assert_eq!(resolved.inner, ratatui::layout::Rect::new(11, 10, 77, 10));
    }

    #[test]
    fn allows_full_terminal_outer_size() {
        let resolved = super::resolve_popup_geometry(
            Some(PopupSize::Percent(100)),
            Some(PopupSize::Percent(100)),
            PanePadding::default(),
            ratatui::layout::Rect::new(4, 2, 100, 30),
        )
        .unwrap();

        assert_eq!(resolved.outer, ratatui::layout::Rect::new(4, 2, 100, 30));
        assert_eq!(resolved.inner, ratatui::layout::Rect::new(5, 3, 97, 28));
    }

    #[test]
    fn enforces_runtime_minimum_terminal_width() {
        let resolved = super::resolve_popup_geometry(
            Some(PopupSize::Cells(4)),
            None,
            PanePadding::default(),
            ratatui::layout::Rect::new(0, 0, 80, 24),
        )
        .unwrap();
        assert_eq!(resolved.outer.width, 6);
        assert_eq!(resolved.inner.width, 4);

        assert!(super::resolve_popup_geometry(
            None,
            None,
            PanePadding::default(),
            ratatui::layout::Rect::new(0, 0, 5, 24),
        )
        .is_none());
    }

    #[test]
    fn padding_shrinks_the_inner_area_inside_the_border() {
        let resolved = super::resolve_popup_geometry(
            Some(PopupSize::Percent(80)),
            Some(PopupSize::Percent(40)),
            PanePadding {
                top: 1,
                right: 2,
                bottom: 3,
                left: 4,
            },
            ratatui::layout::Rect::new(0, 0, 100, 30),
        )
        .unwrap();

        // The outer frame is unchanged; only the terminal area moves in.
        assert_eq!(resolved.outer, ratatui::layout::Rect::new(10, 9, 80, 12));
        // 78 inner columns - 4 left - 2 right = 72, minus the spacer column.
        assert_eq!(resolved.inner, ratatui::layout::Rect::new(15, 11, 71, 6));
    }

    #[test]
    fn padding_is_clamped_so_content_always_survives() {
        let resolved = super::resolve_popup_geometry(
            Some(PopupSize::Cells(6)),
            Some(PopupSize::Cells(4)),
            PanePadding {
                top: 40,
                right: 40,
                bottom: 40,
                left: 40,
            },
            ratatui::layout::Rect::new(0, 0, 80, 24),
        )
        .unwrap();

        assert_eq!(resolved.outer, ratatui::layout::Rect::new(37, 10, 6, 4));
        assert_eq!(resolved.inner.width, 1);
        assert_eq!(resolved.inner.height, 1);
    }
}
