use egui::{Id, Rect, Response, Sense, Ui, vec2};
use rstar::{PointDistance, RTree, RTreeObject};
use std::cell::RefCell;
use std::sync::Arc;
use walkers::{MapMemory, Plugin, Position, Projector, lon_lat, mercator};

/// [`Plugin`] which shows places on the map. Place can be any type that implements the [`Place`]
/// trait.
pub struct Places<T>
where
    T: Place,
{
    places: Vec<T>,
}

impl<T> Places<T>
where
    T: Place,
{
    pub fn new(places: Vec<T>) -> Self {
        Self { places }
    }
}

impl<T> Plugin for Places<T>
where
    T: Place + 'static,
{
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        _response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for place in &self.places {
            place.draw(ui, projector);
        }
    }
}

pub trait Place {
    fn position(&self) -> Position;
    fn draw(&self, ui: &Ui, projector: &Projector);
}

/// A group of places that can be drawn together on the map.
pub trait Group {
    fn draw<T: Place>(&self, places: &[&T], position: Position, projector: &Projector, ui: &mut Ui);
}

/// Similar to [`Places`], but groups places that are close together and draws them as a
/// single [`Group`].
pub struct GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    places: Vec<T>,
    group: G,
}

impl<T, G> GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    pub fn new(places: Vec<T>, group: G) -> Self {
        Self { places, group }
    }

    /// Handle user interactions. Returns whether group should be expanded.
    fn interact(&self, position: Position, projector: &Projector, ui: &Ui, id: Id) -> bool {
        let screen_position = projector.project(position);
        let rect = Rect::from_center_size(screen_position.to_pos2(), vec2(50., 50.));
        let response = ui.interact(rect, id, Sense::click());

        if response.clicked() {
            // Toggle the visibility of the group when clicked.
            ui.ctx().memory_mut(|m| {
                let expand = m.data.get_temp::<bool>(id).unwrap_or(false);
                m.data.insert_temp(id, !expand);
                expand
            })
        } else {
            ui.ctx()
                .memory(|m| m.data.get_temp::<bool>(id).unwrap_or(false))
        }
    }
}

impl<T, G> Plugin for GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        _response: &Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        for (idx, places) in groups(&self.places, projector).iter().enumerate() {
            let id = ui.id().with(idx);
            let position = center(&places.iter().map(|p| p.position()).collect::<Vec<_>>());
            let expand = self.interact(position, projector, ui, id);

            if places.len() >= 2 && !expand {
                self.group.draw(places, position, projector, ui);
            } else {
                for place in places {
                    place.draw(ui, projector);
                }
            }
        }
    }
}

/// Group places that are close together.
fn groups<'a, T>(places: &'a [T], projector: &Projector) -> Vec<Vec<&'a T>>
where
    T: Place,
{
    let mut groups: Vec<Vec<&T>> = Vec::new();

    for place in places {
        if let Some(group) = groups.iter_mut().find(|g| {
            g.iter()
                .all(|p| distance_projected(place.position(), p.position(), projector) < 50.0)
        }) {
            group.push(place);
        } else {
            groups.push(vec![place]);
        }
    }

    groups
}

/// Calculate the distance between two positions after being projected onto the screen.
fn distance_projected(p1: Position, p2: Position, projector: &Projector) -> f32 {
    let screen_p1 = projector.project(p1).to_pos2();
    let screen_p2 = projector.project(p2).to_pos2();
    (screen_p1 - screen_p2).length()
}

fn center(positions: &[Position]) -> Position {
    if positions.is_empty() {
        Position::default()
    } else {
        let sum = positions
            .iter()
            .fold(Position::default(), |acc, &p| acc + p);
        sum / positions.len() as f64
    }
}

#[derive(Clone, Debug)]
pub struct GroupedPlacesTreeSettings {
    pub screen_radius_px: Option<f32>,
    pub geo_radius_deg: f64,
    pub viewport_only: bool,
    pub include_offscreen_neighbors: bool,
    pub max_group_size: Option<usize>,
}

impl Default for GroupedPlacesTreeSettings {
    fn default() -> Self {
        Self {
            screen_radius_px: None,
            geo_radius_deg: 0.001,
            viewport_only: false,
            include_offscreen_neighbors: true,
            max_group_size: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Pt {
    idx: usize,
    lon: f64,
    lat: f64,
}

impl RTreeObject for Pt {
    type Envelope = rstar::AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_point([self.lon, self.lat])
    }
}

impl PointDistance for Pt {
    #[inline]
    fn distance_2(&self, p: &[f64; 2]) -> f64 {
        let dx = self.lon - p[0];
        let dy = self.lat - p[1];
        dx * dx + dy * dy
    }
}

fn interact_cluster(
    ui: &Ui,
    projector: &Projector,
    center: Position,
    cluster_id: egui::Id,
    hitbox_px: f32,
) -> bool {
    let screen = projector.project(center).to_pos2();
    let rect = egui::Rect::from_center_size(screen, egui::vec2(hitbox_px, hitbox_px));
    let resp = ui.interact(rect, cluster_id, egui::Sense::click());

    if resp.clicked() {
        ui.ctx().memory_mut(|m| {
            let v = m.data.get_temp::<bool>(cluster_id).unwrap_or(false);
            m.data.insert_temp(cluster_id, !v);
            v
        })
    } else {
        ui.ctx()
            .memory(|m| m.data.get_temp::<bool>(cluster_id).unwrap_or(false))
    }
}

#[derive(Clone)]
pub struct GroupedPlacesTree<T: Place, G: Group> {
    places: Arc<Vec<T>>,
    group: Arc<G>,
    settings: GroupedPlacesTreeSettings,
    rtree: Arc<RTree<Pt>>,
    screen_positions: RefCell<Vec<egui::Pos2>>,
}

impl<T: Place, G: Group> GroupedPlacesTree<T, G> {
    pub fn new(places: Vec<T>, group: G) -> Self {
        let rtree = build_rtree(&places);
        Self {
            places: Arc::new(places),
            group: Arc::new(group),
            settings: GroupedPlacesTreeSettings::default(),
            rtree: Arc::new(rtree),
            screen_positions: RefCell::new(Vec::new()),
        }
    }

    pub fn with_settings(mut self, s: GroupedPlacesTreeSettings) -> Self {
        self.settings = s;
        self
    }

    pub fn with_screen_radius_px(mut self, px: f32) -> Self {
        self.settings.screen_radius_px = Some(px);
        self
    }

    pub fn with_geo_radius_deg(mut self, deg: f64) -> Self {
        self.settings.geo_radius_deg = deg;
        self
    }

    pub fn with_max_group_size(mut self, cap: Option<usize>) -> Self {
        self.settings.max_group_size = cap;
        self
    }

    pub fn viewport_only(mut self, on: bool) -> Self {
        self.settings.viewport_only = on;
        self
    }

    pub fn include_offscreen_neighbors(mut self, on: bool) -> Self {
        self.settings.include_offscreen_neighbors = on;
        self
    }

    pub fn update_points(&mut self, places: Vec<T>) {
        self.places = Arc::new(places);
        self.rtree = Arc::new(build_rtree(&self.places));
        self.screen_positions.borrow_mut().clear();
    }

    fn px_per_deg(&self, memory: &MapMemory, seed: [f64; 2]) -> (f64, f64) {
        let zoom = memory.zoom();
        let pos = lon_lat(seed[0], seed[1]);
        let base = mercator::project(pos, zoom);
        const D: f64 = 1e-4;
        let lon_shift = mercator::project(lon_lat(seed[0] + D, seed[1]), zoom);
        let lat_shift = mercator::project(lon_lat(seed[0], seed[1] + D), zoom);
        let px_per_deg_lon = ((lon_shift.x() - base.x()).abs() / D).max(1e-9);
        let px_per_deg_lat = ((lat_shift.y() - base.y()).abs() / D).max(1e-9);
        (px_per_deg_lon, px_per_deg_lat)
    }

    fn px_to_deg_at(&self, memory: &MapMemory, seed: [f64; 2], r_px: f32) -> f64 {
        let (px_per_deg_lon, px_per_deg_lat) = self.px_per_deg(memory, seed);
        let r_px = r_px as f64;
        let dlon = r_px / px_per_deg_lon;
        let dlat = r_px / px_per_deg_lat;
        dlon.hypot(dlat)
    }

    fn deg_to_px_at(&self, memory: &MapMemory, seed: [f64; 2], r_deg: f64) -> f32 {
        let (px_per_deg_lon, px_per_deg_lat) = self.px_per_deg(memory, seed);
        let px_lon = r_deg * px_per_deg_lon;
        let px_lat = r_deg * px_per_deg_lat;
        px_lon.hypot(px_lat) as f32
    }

    fn visit_clusters_with_cache<F>(
        &self,
        response_rect: egui::Rect,
        memory: &MapMemory,
        screen_positions: &[egui::Pos2],
        mut handle: F,
    ) where
        F: FnMut(usize, &[usize], Position),
    {
        let s = &self.settings;
        let places = &self.places;
        let rtree = &self.rtree;

        let mut visited = vec![false; places.len()];
        let mut scratch: Vec<usize> = Vec::with_capacity(64);

        let mut screen_rect = response_rect;
        if s.viewport_only {
            if let Some(px) = s.screen_radius_px {
                if s.include_offscreen_neighbors && px > 0.0 {
                    screen_rect = screen_rect.expand(px);
                }
            }
        }

        for seed in rtree.iter() {
            if visited[seed.idx] {
                continue;
            }
            scratch.clear();

            let seed_screen = screen_positions[seed.idx];
            if s.viewport_only && !screen_rect.contains(seed_screen) {
                continue;
            }

            let (query_r_deg, r_px_check) = if let Some(px) = s.screen_radius_px {
                (self.px_to_deg_at(memory, [seed.lon, seed.lat], px), px)
            } else {
                (
                    s.geo_radius_deg,
                    self.deg_to_px_at(memory, [seed.lon, seed.lat], s.geo_radius_deg),
                )
            };

            for n in rtree.locate_within_distance([seed.lon, seed.lat], query_r_deg) {
                if visited[n.idx] {
                    continue;
                }

                let sp = screen_positions[n.idx];
                if s.viewport_only && !screen_rect.contains(sp) {
                    continue;
                }
                if (sp - seed_screen).length() > r_px_check {
                    continue;
                }

                scratch.push(n.idx);
            }

            if let Some(cap) = s.max_group_size {
                if scratch.len() > cap {
                    scratch.truncate(cap);
                }
            }
            if scratch.is_empty() {
                continue;
            }
            for &i in &scratch {
                visited[i] = true;
            }

            let (sum_lon, sum_lat) = scratch.iter().fold((0.0, 0.0), |acc, &i| {
                let p = places[i].position();
                (acc.0 + p.x(), acc.1 + p.y())
            });
            let center = lon_lat(
                sum_lon / scratch.len() as f64,
                sum_lat / scratch.len() as f64,
            );

            handle(seed.idx, &scratch, center);
        }
    }

    pub fn draw_once(
        &self,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        memory: &MapMemory,
    ) {
        self.draw_with_stats(ui, response, projector, memory);
    }

    pub fn draw_with_stats(
        &self,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        memory: &MapMemory,
    ) -> (usize, usize) {
        let mut clusters = 0usize;
        let mut max_size = 0usize;
        let mut cache = self.screen_positions.borrow_mut();
        if cache.len() != self.places.len() {
            cache.resize(self.places.len(), egui::Pos2::new(0.0, 0.0));
        }
        for (pos, place) in cache.iter_mut().zip(self.places.iter()) {
            *pos = projector.project(place.position()).to_pos2();
        }
        self.visit_clusters_with_cache(
            response.rect,
            memory,
            &cache,
            |seed_idx, members, center| {
                const HITBOX_PX: f32 = 50.0;
                let cluster_id = ui.id().with(("rstar_cluster", seed_idx));
                let expand = interact_cluster(ui, projector, center, cluster_id, HITBOX_PX);

                if members.len() >= 2 && !expand {
                    let refs: Vec<&T> = members.iter().map(|&i| &self.places[i]).collect();
                    self.group.draw(&refs, center, projector, ui);
                } else {
                    for &idx in members {
                        self.places[idx].draw(ui, projector);
                    }
                }

                clusters += 1;
                max_size = max_size.max(members.len());
            },
        );
        (clusters, max_size)
    }

    pub fn cluster_stats(
        &self,
        rect: egui::Rect,
        projector: &Projector,
        memory: &MapMemory,
    ) -> (usize, usize) {
        let mut clusters = 0usize;
        let mut max_size = 0usize;
        let mut cache = self.screen_positions.borrow_mut();
        if cache.len() != self.places.len() {
            cache.resize(self.places.len(), egui::Pos2::new(0.0, 0.0));
        }
        for (pos, place) in cache.iter_mut().zip(self.places.iter()) {
            *pos = projector.project(place.position()).to_pos2();
        }
        self.visit_clusters_with_cache(rect, memory, &cache, |_, members, _| {
            clusters += 1;
            max_size = max_size.max(members.len());
        });
        (clusters, max_size)
    }
}

impl<T: Place, G: Group> Plugin for GroupedPlacesTree<T, G> {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector,
        memory: &MapMemory,
    ) {
        self.draw_once(ui, response, projector, memory);
    }
}

#[inline]
fn build_rtree<T: Place>(places: &[T]) -> RTree<Pt> {
    let pts: Vec<Pt> = places
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let pos = p.position();
            Pt {
                idx,
                lon: pos.x(),
                lat: pos.y(),
            }
        })
        .collect();
    RTree::bulk_load(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect, Vec2};

    #[derive(Clone)]
    struct DummyPlace(Position);

    impl Place for DummyPlace {
        fn position(&self) -> Position {
            self.0
        }

        fn draw(&self, _ui: &Ui, _projector: &Projector) {}
    }

    #[derive(Clone)]
    struct DummyGroup;

    impl Group for DummyGroup {
        fn draw<T: Place>(
            &self,
            _places: &[&T],
            _position: Position,
            _projector: &Projector,
            _ui: &mut Ui,
        ) {
        }
    }

    fn projector_for_zoom(zoom: f64) -> (Rect, MapMemory, Projector) {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(512.0));
        let mut memory = MapMemory::default();
        memory.set_zoom(zoom).unwrap();
        let projector = Projector::new(rect, &memory, lon_lat(0.0, 0.0));
        (rect, memory, projector)
    }

    #[test]
    fn clusters_split_when_zoomed_in() {
        let places = vec![
            DummyPlace(lon_lat(0.0, 0.0)),
            DummyPlace(lon_lat(0.01, 0.0)),
        ];
        let tree = GroupedPlacesTree::new(places, DummyGroup)
            .with_screen_radius_px(50.0)
            .viewport_only(false);

        let (rect_far, mem_far, proj_far) = projector_for_zoom(8.0);
        let (clusters_far, max_far) = tree.cluster_stats(rect_far, &proj_far, &mem_far);
        assert_eq!(clusters_far, 1);
        assert_eq!(max_far, 2);

        let (rect_near, mem_near, proj_near) = projector_for_zoom(18.0);
        let (clusters_near, max_near) = tree.cluster_stats(rect_near, &proj_near, &mem_near);
        assert_eq!(clusters_near, 2);
        assert_eq!(max_near, 1);
    }

    #[test]
    fn calculating_center() {
        assert_eq!(
            center(&[
                Position::new(0.0, 0.0),
                Position::new(10.0, 10.0),
                Position::new(20.0, 20.0),
            ]),
            Position::new(10.0, 10.0)
        );

        assert_eq!(
            center(&[
                Position::new(0.0, 0.0),
                Position::new(10.0, 0.0),
                Position::new(0.0, 10.0),
                Position::new(10.0, 10.0),
            ]),
            Position::new(5.0, 5.0)
        );

        assert_eq!(
            center(&[
                Position::new(10.0, 10.0),
                Position::new(-10.0, -10.0),
                Position::new(-10.0, 10.0),
                Position::new(10.0, -10.0),
            ]),
            Position::new(0.0, 0.0)
        );
    }
}
