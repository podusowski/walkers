use egui::{Id, Rect, Response, Sense, Ui, vec2};
use rstar::{PointDistance, RTree, RTreeObject};
use std::cell::RefCell;
use std::sync::Arc;
use walkers::{Plugin, Position, Projector};

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
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &Projector<'_>) {
        for place in &self.places {
            place.draw(ui, projector);
        }
    }
}

pub trait Place {
    fn position(&self) -> Position;
    fn draw(&self, ui: &Ui, projector: &Projector<'_>);
}

/// A group of places that can be drawn together on the map.
pub trait Group {
    fn draw<T: Place>(
        &self,
        places: &[&T],
        position: Position,
        projector: &Projector<'_>,
        ui: &mut Ui,
    );
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
    fn interact(&self, position: Position, projector: &Projector<'_>, ui: &Ui, id: Id) -> bool {
        let screen_position = projector.project(position);
        let rect = Rect::from_center_size(screen_position.to_pos2(), vec2(50., 50.));
        let response = ui.interact(rect, id, Sense::click());

        if response.clicked() {
            // Toggle the visibility of the group when clicked.
            ui.memory_mut(|m| {
                let expand = m.data.get_temp::<bool>(id).unwrap_or(false);
                m.data.insert_temp(id, !expand);
                expand
            })
        } else {
            ui.memory(|m| m.data.get_temp::<bool>(id).unwrap_or(false))
        }
    }
}

impl<T, G> Plugin for GroupedPlaces<T, G>
where
    T: Place,
    G: Group,
{
    fn run(self: Box<Self>, ui: &mut Ui, _response: &Response, projector: &Projector<'_>) {
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
fn groups<'a, T>(places: &'a [T], projector: &Projector<'_>) -> Vec<Vec<&'a T>>
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
fn distance_projected(p1: Position, p2: Position, projector: &Projector<'_>) -> f32 {
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
    /// Cluster radius in screen pixels. When set, overrides `world_radius`.
    pub screen_radius_px: Option<f32>,
    /// Euclidean cluster radius in the input coordinate units: degrees for geographic positions,
    /// or linear units (meters for [`walkers::PlanarProjection`]) for planar positions.
    /// This is not a geodesic distance.
    pub world_radius: f64,
    /// Only start clusters from places inside the viewport.
    pub viewport_only: bool,
    /// Allow a visible cluster to include places outside the viewport.
    pub include_offscreen_neighbors: bool,
    pub max_group_size: Option<usize>,
}

impl Default for GroupedPlacesTreeSettings {
    fn default() -> Self {
        Self {
            screen_radius_px: None,
            world_radius: 0.001,
            viewport_only: false,
            include_offscreen_neighbors: true,
            max_group_size: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pt {
    idx: usize,
    position: [f64; 2],
}

impl RTreeObject for Pt {
    type Envelope = rstar::AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_point(self.position)
    }
}

impl PointDistance for Pt {
    #[inline]
    fn distance_2(&self, p: &[f64; 2]) -> f64 {
        let dx = self.position[0] - p[0];
        let dy = self.position[1] - p[1];
        dx * dx + dy * dy
    }
}

/// Projected pixels do not change when the map is panned. Compare the coordinates themselves
/// so changing projections (including their parameters) or zoom rebuilds the index as needed.
#[derive(Clone, Default)]
struct ProjectedPlaces {
    points: Vec<Pt>,
    rtree: Arc<RTree<Pt>>,
}

impl ProjectedPlaces {
    fn update<T: Place>(&mut self, places: &[T], projector: &Projector<'_>) {
        let mut changed = self.points.len() != places.len();
        self.points.resize(
            places.len(),
            Pt {
                idx: 0,
                position: [0.0, 0.0],
            },
        );

        for (idx, (cached, place)) in self.points.iter_mut().zip(places).enumerate() {
            let pixels = projector
                .projection()
                .position_to_pixels(place.position(), projector.zoom());
            let point = Pt {
                idx,
                position: [pixels.x(), pixels.y()],
            };
            changed |= *cached != point;
            *cached = point;
        }

        if changed {
            self.rtree = Arc::new(RTree::bulk_load(self.points.clone()));
        }
    }
}

fn interact_cluster(
    ui: &Ui,
    projector: &Projector<'_>,
    center: Position,
    cluster_id: egui::Id,
    hitbox_px: f32,
) -> bool {
    let screen = projector.project(center).to_pos2();
    let rect = egui::Rect::from_center_size(screen, egui::vec2(hitbox_px, hitbox_px));
    let resp = ui.interact(rect, cluster_id, egui::Sense::click());

    if resp.clicked() {
        ui.memory_mut(|m| {
            let v = m.data.get_temp::<bool>(cluster_id).unwrap_or(false);
            m.data.insert_temp(cluster_id, !v);
            v
        })
    } else {
        ui.memory(|m| m.data.get_temp::<bool>(cluster_id).unwrap_or(false))
    }
}

/// Groups places using an R-tree in world coordinates or projected pixel coordinates.
/// The current [`Projector`] supplies the projection, so the same tree can be drawn on
/// maps with different projections.
#[derive(Clone)]
pub struct GroupedPlacesTree<T: Place, G: Group> {
    places: Arc<Vec<T>>,
    group: Arc<G>,
    settings: GroupedPlacesTreeSettings,
    rtree: Arc<RTree<Pt>>,
    screen_positions: RefCell<Vec<egui::Pos2>>,
    projected: RefCell<ProjectedPlaces>,
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
            projected: RefCell::new(ProjectedPlaces::default()),
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

    /// Cluster by Euclidean distance in the places' input coordinate units, instead of pixels.
    pub fn with_world_radius(mut self, radius: f64) -> Self {
        self.settings.world_radius = radius;
        self.settings.screen_radius_px = None;
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
        *self.projected.borrow_mut() = ProjectedPlaces::default();
    }

    fn visit_clusters<F>(&self, response_rect: egui::Rect, projector: &Projector<'_>, mut handle: F)
    where
        F: FnMut(usize, &[usize], Position),
    {
        let s = &self.settings;
        let places = &self.places;

        let mut screen_positions = self.screen_positions.borrow_mut();
        screen_positions.resize(places.len(), egui::Pos2::ZERO);
        for (screen, place) in screen_positions.iter_mut().zip(places.iter()) {
            *screen = projector.project(place.position()).to_pos2();
        }

        let mut projected = self.projected.borrow_mut();
        let (rtree, radius) = if let Some(px) = s.screen_radius_px {
            projected.update(places, projector);
            (&*projected.rtree, f64::from(px))
        } else {
            (&*self.rtree, s.world_radius)
        };
        if !radius.is_finite() || radius < 0.0 {
            return;
        }

        let mut visited = vec![false; places.len()];
        let mut scratch: Vec<usize> = Vec::with_capacity(64);

        // Keep seed order independent of projection and zoom, preserving cluster IDs while panning.
        for seed in self.rtree.iter() {
            if visited[seed.idx] {
                continue;
            }
            scratch.clear();

            let seed_screen = screen_positions[seed.idx];
            if s.viewport_only && !response_rect.contains(seed_screen) {
                continue;
            }

            let query_position = if s.screen_radius_px.is_some() {
                projected.points[seed.idx].position
            } else {
                seed.position
            };

            // rstar expects squared distance, in the same units as this index's coordinates.
            for n in rtree.locate_within_distance(query_position, radius * radius) {
                if visited[n.idx] {
                    continue;
                }

                let sp = screen_positions[n.idx];
                if s.viewport_only && !s.include_offscreen_neighbors && !response_rect.contains(sp)
                {
                    continue;
                }

                scratch.push(n.idx);
            }

            if let Some(cap) = s.max_group_size
                && scratch.len() > cap
            {
                scratch.truncate(cap);
            }
            if scratch.is_empty() {
                continue;
            }
            for &i in &scratch {
                visited[i] = true;
            }

            let (sum_x, sum_y) = scratch.iter().fold((0.0, 0.0), |acc, &i| {
                let p = places[i].position();
                (acc.0 + p.x(), acc.1 + p.y())
            });
            let center = Position::new(sum_x / scratch.len() as f64, sum_y / scratch.len() as f64);

            handle(seed.idx, &scratch, center);
        }
    }

    pub fn draw_once(&self, ui: &mut Ui, response: &Response, projector: &Projector<'_>) {
        self.draw_with_stats(ui, response, projector);
    }

    pub fn draw_with_stats(
        &self,
        ui: &mut Ui,
        response: &Response,
        projector: &Projector<'_>,
    ) -> (usize, usize) {
        let mut clusters = 0usize;
        let mut max_size = 0usize;
        self.visit_clusters(response.rect, projector, |seed_idx, members, center| {
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
        });
        (clusters, max_size)
    }

    pub fn cluster_stats(&self, rect: egui::Rect, projector: &Projector<'_>) -> (usize, usize) {
        let mut clusters = 0usize;
        let mut max_size = 0usize;
        self.visit_clusters(rect, projector, |_, members, _| {
            clusters += 1;
            max_size = max_size.max(members.len());
        });
        (clusters, max_size)
    }
}

impl<T, G> Plugin for GroupedPlacesTree<T, G>
where
    T: Place,
    G: Group,
{
    fn run(self: Box<Self>, ui: &mut Ui, response: &Response, projector: &Projector<'_>) {
        self.draw_once(ui, response, projector);
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
                position: [pos.x(), pos.y()],
            }
        })
        .collect();
    RTree::bulk_load(pts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect, Vec2};
    use walkers::{MapMemory, Projector, lon_lat};

    #[derive(Clone)]
    struct DummyPlace(Position);

    impl Place for DummyPlace {
        fn position(&self) -> Position {
            self.0
        }

        fn draw(&self, _ui: &Ui, _projector: &Projector<'_>) {}
    }

    #[derive(Clone)]
    struct DummyGroup;

    impl Group for DummyGroup {
        fn draw<T: Place>(
            &self,
            _places: &[&T],
            _position: Position,
            _projector: &Projector<'_>,
            _ui: &mut Ui,
        ) {
        }
    }

    fn projector_for_zoom(zoom: f64, memory: &mut MapMemory) -> (Rect, Projector<'_>) {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(512.0));
        memory.set_zoom(zoom).unwrap();
        let projector = Projector::new(rect, memory, lon_lat(0.0, 0.0));
        (rect, projector)
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

        let mut far_memory = MapMemory::default();
        let (rect_far, proj_far) = projector_for_zoom(8.0, &mut far_memory);
        let (clusters_far, max_far) = tree.cluster_stats(rect_far, &proj_far);
        assert_eq!(clusters_far, 1);
        assert_eq!(max_far, 2);

        let mut near_memory = MapMemory::default();
        let (rect_near, proj_near) = projector_for_zoom(18.0, &mut near_memory);
        let (clusters_near, max_near) = tree.cluster_stats(rect_near, &proj_near);
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
