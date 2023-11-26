//! Few common places in the city of Wrocław, used in the example app.

use walkers::Position;

/// Main train station of the city of Wrocław.
/// https://en.wikipedia.org/wiki/Wroc%C5%82aw_G%C5%82%C3%B3wny_railway_station
pub fn wroclaw_glowny() -> Position {
    Position::from_lon_lat(17.03664, 51.09916)
}

/// Taking a public bus (line 106) is probably the cheapest option to get from
/// the train station to the airport.
/// https://www.wroclaw.pl/en/how-and-where-to-buy-public-transport-tickets-in-wroclaw
pub fn dworcowa_bus_stop() -> Position {
    Position::from_lon_lat(17.03940, 51.10005)
}

/// Musical Theatre Capitol.
/// https://www.teatr-capitol.pl/
pub fn capitol() -> Position {
    Position::from_lon_lat(17.03018, 51.10073)
}

/// Shopping center, and the main intercity bus station.
pub fn wroclavia() -> Position {
    Position::from_lon_lat(17.03471, 51.09648)
}
