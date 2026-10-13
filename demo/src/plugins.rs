use egui::Color32;
use walkers::Plugin;
use walkers_extras::{
    GroupedPlaces, LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle, LabeledSymbolStyle,
    Symbol,
};

use crate::places;

/// Creates a built-in [`GroupedPlaces`] plugin populated with some predefined places.
pub fn places() -> impl Plugin {
    GroupedPlaces::new(
        vec![
            LabeledSymbol {
                position: places::wroclaw_glowny(),
                label: "Wrocław Główny\ntrain station".to_owned(),
                symbol: Some(Symbol::Circle("🚆".to_string())),
                style: LabeledSymbolStyle {
                    symbol_size: 25.,
                    ..Default::default()
                },
            },
            LabeledSymbol {
                position: places::dworcowa_bus_stop(),
                label: "Bus stop".to_owned(),
                symbol: Some(Symbol::TwoCorners(String::from("🚌"))),
                style: LabeledSymbolStyle {
                    label_corner_radius: 2.,
                    symbol_size: 18.,
                    symbol_background: Color32::WHITE.gamma_multiply(0.4),
                    ..Default::default()
                },
            },
            LabeledSymbol {
                position: places::rynek(),
                label: "Rynek".to_owned(),
                symbol: None,
                style: LabeledSymbolStyle::default(),
            },
        ],
        LabeledSymbolGroup {
            style: LabeledSymbolGroupStyle::default(),
        },
    )
}
