//! Phase 5 — Edge label placement.
//!
//! The routing phase calls [`seek_label_anchor`] right after committing
//! each edge's polyline. The function searches the route's neighbourhood
//! for an anchor whose bounding box is free on the [`GridOccupancy`]
//! and returns it; the caller then reserves the bounding box so
//! subsequent A* routes treat the label as impassable terrain.
//!
//! Falls back to the unconstrained midpoint placement when no candidate
//! anchor is collision-free, so labels are always rendered.

use crate::ast::{EdgeLabelAnchor, Point, TextExtents};
use crate::routing::GridOccupancy;

/// Compute the unconstrained "default" anchor for a route: midpoint with
/// a perpendicular offset from the local segment direction.
pub fn place_edge_label(route: &[Point]) -> Option<EdgeLabelAnchor> {
    if route.len() < 2 {
        return None;
    }
    let (cx, cy, dx, dy) = midpoint_orientation(route);
    let (x, y, anchor) = if dx >= dy {
        (cx, cy - 10, "middle")
    } else {
        (cx + 10, cy, "start")
    };
    Some(EdgeLabelAnchor { x, y, anchor })
}

/// Return the route midpoint and the absolute orientation deltas around it.
fn midpoint_orientation(route: &[Point]) -> (i32, i32, i32, i32) {
    let mid = route.len() / 2;
    let p = route[mid];
    let prev = route[mid.saturating_sub(1)];
    let next = route[(mid + 1).min(route.len() - 1)];
    let dx = (next.x - prev.x).abs();
    let dy = (next.y - prev.y).abs();
    (p.x, p.y, dx, dy)
}

/// Search the local neighbourhood of a route for a free anchor for an
/// edge label of the given extents.
///
/// Strategy: try the midpoint with several perpendicular offsets, then
/// try anchors at neighbouring waypoints. Returns the first anchor whose
/// bounding box does not collide with anything on the occupancy grid;
/// falls back to the default anchor if nothing fits.
pub fn seek_label_anchor(
    route: &[Point],
    extents: TextExtents,
    occupancy: &GridOccupancy,
) -> Option<EdgeLabelAnchor> {
    if route.len() < 2 {
        return None;
    }

    let default = place_edge_label(route)?;
    if !occupancy.label_collides(default, extents) {
        return Some(default);
    }

    // Sample candidates: per-waypoint perpendicular offsets at increasing
    // distance, in deterministic order.
    let offsets: [i32; 6] = [12, 18, 24, 30, 36, 42];
    let mid = route.len() / 2;
    let waypoint_order: Vec<usize> = waypoint_search_order(route.len(), mid);

    for &i in &waypoint_order {
        let (cx, cy, dx, dy) = local_orientation_at(route, i);
        let horizontal = dx >= dy;
        for &mag in &offsets {
            for &sign in &[-1, 1] {
                let (x, y, anchor) = if horizontal {
                    (cx, cy + sign * mag, "middle")
                } else {
                    (cx + sign * mag, cy, if sign > 0 { "start" } else { "end" })
                };
                let candidate = EdgeLabelAnchor { x, y, anchor };
                if !occupancy.label_collides(candidate, extents) {
                    return Some(candidate);
                }
            }
        }
    }

    Some(default)
}

/// Local orientation around a single waypoint; mirrors `midpoint_orientation`
/// but for an arbitrary index.
fn local_orientation_at(route: &[Point], i: usize) -> (i32, i32, i32, i32) {
    let p = route[i];
    let prev = route[i.saturating_sub(1)];
    let next = route[(i + 1).min(route.len() - 1)];
    let dx = (next.x - prev.x).abs();
    let dy = (next.y - prev.y).abs();
    (p.x, p.y, dx, dy)
}

/// Yield waypoint indices to probe in order of increasing distance from
/// the midpoint: mid, mid±1, mid±2, …
fn waypoint_search_order(n: usize, mid: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(n);
    out.push(mid);
    let mut step = 1usize;
    while out.len() < n {
        if mid >= step {
            out.push(mid - step);
        }
        if mid + step < n {
            out.push(mid + step);
        }
        step += 1;
        if step > n {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    #[test]
    fn empty_route_has_no_anchor() {
        assert_eq!(place_edge_label(&[]), None);
        assert_eq!(place_edge_label(&[pt(0, 0)]), None);
    }

    #[test]
    fn horizontal_segment_places_above_midpoint() {
        let route = [pt(0, 50), pt(100, 50), pt(200, 50)];
        let a = place_edge_label(&route).unwrap();
        assert_eq!(a.anchor, "middle");
        assert_eq!(a.x, 100);
        assert_eq!(a.y, 40);
    }

    #[test]
    fn vertical_segment_places_to_right_of_midpoint() {
        let route = [pt(50, 0), pt(50, 100), pt(50, 200)];
        let a = place_edge_label(&route).unwrap();
        assert_eq!(a.anchor, "start");
        assert_eq!(a.x, 60);
        assert_eq!(a.y, 100);
    }

    #[test]
    fn seek_anchor_falls_back_to_default_when_grid_is_empty() {
        let route = [pt(0, 50), pt(100, 50), pt(200, 50)];
        let extents = TextExtents { width: 30.0, height: 12.0 };
        let d = crate::parser::parse_dsl("A -> B\n").unwrap();
        let occ = GridOccupancy::new(&d);
        let anchor = seek_label_anchor(&route, extents, &occ).unwrap();
        assert_eq!(anchor.x, 100);
    }
}
