//! Few common places in the city of Wrocław, used in the example app.

use walkers::{lon_lat, Position};

/// Main train station of the city of Wrocław.
/// https://en.wikipedia.org/wiki/Wroc%C5%82aw_G%C5%82%C3%B3wny_railway_station
pub fn wroclaw_glowny() -> Position {
    lon_lat(17.03664, 51.09916)
}

/// Taking a public bus (line 106) is probably the cheapest option to get from
/// the train station to the airport.
/// https://www.wroclaw.pl/en/how-and-where-to-buy-public-transport-tickets-in-wroclaw
pub fn dworcowa_bus_stop() -> Position {
    lon_lat(17.03940, 51.10005)
}

/// Musical Theatre Capitol.
/// https://www.teatr-capitol.pl/
pub fn capitol() -> Position {
    lon_lat(17.03018, 51.10073)
}

/// Main square of the city, with many restaurants and historical buildings.
pub fn rynek() -> Position {
    lon_lat(17.032094, 51.110090)
}

pub fn bastion_sakwowy() -> Position {
    lon_lat(17.0377, 51.1033)
}
