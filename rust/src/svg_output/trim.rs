//! Route trimming so arrowheads aren't covered by node rectangles.
//!
//! The router emits routes that start at the source's port cell (a fixed
//! 30 px from node centre) and finish at the target's port cell. For nodes
//! whose rectangles are wider or taller than the port offset (because the
//! label needed extra padding), those endpoints actually live *inside* the
//! node rectangle. Since the rectangle is drawn on top of the edge in
//! z-order, any arrowhead at the route's last point would be hidden.
//!
//! These helpers clip the head and tail of the route against the node
//! rectangles so the visible polyline ends exactly at the rectangle edge.

use crate::ast::{Node, Point};

/// Strict containment test (open rectangle interior).
pub(super) fn point_inside_rect(p: Point, center: Point, half_w: i32, half_h: i32) -> bool {
    p.x > center.x - half_w
        && p.x < center.x + half_w
        && p.y > center.y - half_h
        && p.y < center.y + half_h
}

/// Given a segment whose `inside` endpoint sits inside an axis-aligned
/// rectangle and `outside` endpoint sits outside it, return the point at
/// which the segment crosses the rectangle boundary.
fn segment_rect_exit(
    inside: Point,
    outside: Point,
    center: Point,
    half_w: i32,
    half_h: i32,
) -> Point {
    let ax = inside.x as f64;
    let ay = inside.y as f64;
    let dx = (outside.x - inside.x) as f64;
    let dy = (outside.y - inside.y) as f64;

    let mut t_exit = 1.0_f64;

    if dx.abs() > 1e-9 {
        for &xb in &[(center.x - half_w) as f64, (center.x + half_w) as f64] {
            let t = (xb - ax) / dx;
            if t > 1e-9 && t < t_exit {
                t_exit = t;
            }
        }
    }
    if dy.abs() > 1e-9 {
        for &yb in &[(center.y - half_h) as f64, (center.y + half_h) as f64] {
            let t = (yb - ay) / dy;
            if t > 1e-9 && t < t_exit {
                t_exit = t;
            }
        }
    }

    Point::new(
        (ax + t_exit * dx).round() as i32,
        (ay + t_exit * dy).round() as i32,
    )
}

/// Trim a routed polyline so its first and last points lie on the
/// boundaries of the source and target node rectangles respectively.
pub(super) fn trim_route_to_node_boundaries(
    route: &[Point],
    src: Option<&Node>,
    tgt: Option<&Node>,
) -> Vec<Point> {
    if route.len() < 2 {
        return route.to_vec();
    }

    let mut points: Vec<Point> = route.to_vec();

    // ---- Source side: trim leading points that sit inside source rect. ----
    if let Some(node) = src {
        if let (Some(pos), Some(size)) = (node.position, node.node_size) {
            let hw = size.half_w();
            let hh = size.half_h();
            // First index with a point outside the rectangle.
            let first_out = points
                .iter()
                .position(|p| !point_inside_rect(*p, pos, hw, hh));
            if let Some(i) = first_out {
                if i > 0 {
                    let exit = segment_rect_exit(points[i - 1], points[i], pos, hw, hh);
                    let mut new_pts = Vec::with_capacity(points.len() - i + 1);
                    new_pts.push(exit);
                    new_pts.extend_from_slice(&points[i..]);
                    points = new_pts;
                }
            }
        }
    }

    // ---- Target side: clip at the first point that enters the target rect. ----
    if let Some(node) = tgt {
        if let (Some(pos), Some(size)) = (node.position, node.node_size) {
            let hw = size.half_w();
            let hh = size.half_h();
            let first_in = points
                .iter()
                .position(|p| point_inside_rect(*p, pos, hw, hh));
            if let Some(i) = first_in {
                if i > 0 {
                    let entry = segment_rect_exit(points[i], points[i - 1], pos, hw, hh);
                    points.truncate(i);
                    points.push(entry);
                }
            }
        }
    }

    points
}
