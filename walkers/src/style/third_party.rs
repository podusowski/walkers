//! Styles written elsewhere, carried along as the JSON they were published as.

use super::Style;

impl Style {
    /// Style based on Protomaps Dark flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_dark() -> Self {
        let style_json = include_str!("../../assets/protomaps-dark.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    /// Style based on Protomaps Dark Vis flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_dark_vis() -> Self {
        let style_json = include_str!("../../assets/protomaps-dark-vis.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    /// Style based on Protomaps Light flavour. Requires Protomaps source.
    ///
    /// <https://docs.protomaps.com/basemaps/flavors>
    pub fn protomaps_light() -> Self {
        let style_json = include_str!("../../assets/protomaps-light.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }

    pub fn openfreemap_bright() -> Self {
        let style_json = include_str!("../../assets/openfreemap-bright.json");
        serde_json::from_str(style_json).expect("failed to parse style JSON")
    }
}
