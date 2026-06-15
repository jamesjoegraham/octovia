//! Parallel lane assignment — a post-routing pass that fans out forward
//! edges that share a straight segment so they don't draw on top of
//! each other.
//!
//! The router emits every forward edge along the most direct centre-line.
//! When several edges happen to share that line (e.g. multiple parallel
//! transitions along the spine of a row), this pass groups them and
//! offsets each one by one grid cell perpendicular to the run, with 45°
//! diagonal connectors bridging from the original node boundary to the
//! offset lane.

use crate::ast::{Diagram, Point};

/// Grid dimension for lane spacing — one grid cell.
const LANE_SPACING: i32 = 10;

/// After all edges are routed, assign lane offsets to parallel forward
/// edges that share the same straight segment. Each lane is offset by
/// one grid cell (10 px) perpendicular to the edge direction. 45°
/// diagonal connectors bridge from the node boundary to the lane.
///
/// Only horizontal and vertical forward edges are lanned — diagonal
/// A* routes and cyclic U-bends keep their existing routing.
pub(crate) fn assign_parallel_lanes(diagram: &mut Diagram) {
    // Group forward edges by their primary straight segment.
    // Horizontal edges group by y; vertical edges group by x.

    struct LaneGroup {
        indices: Vec<usize>,
        is_horizontal: bool,
        fixed_coord: i32,
        from_min: i32,
        to_max: i32,
    }

    let mut lanes: Vec<LaneGroup> = Vec::new();

    for (i, edge) in diagram.edges.iter().enumerate() {
        if edge.is_cyclic || edge.route.len() < 2 {
            continue;
        }

        // Classify: all route points in a straight line?
        let xs: Vec<i32> = edge.route.iter().map(|p| p.x).collect();
        let ys: Vec<i32> = edge.route.iter().map(|p| p.y).collect();
        let all_same_x = xs.iter().all(|&x| x == xs[0]);
        let all_same_y = ys.iter().all(|&y| y == ys[0]);

        if !all_same_x && !all_same_y {
            continue; // not a straight line
        }

        // Compute along-track bounds (from source centre to target centre)
        let (fixed_coord, a_min, a_max) = if all_same_y {
            (ys[0], xs[0].min(xs[xs.len() - 1]), xs[0].max(xs[xs.len() - 1]))
        } else {
            (xs[0], ys[0].min(ys[ys.len() - 1]), ys[0].max(ys[ys.len() - 1]))
        };

        let is_horizontal = all_same_y;

        // Try to merge into an existing lane that shares the same
        // segment (overlapping x/y ranges).
        let mut merged = false;
        for lane in &mut lanes {
            if lane.is_horizontal != is_horizontal || lane.fixed_coord != fixed_coord {
                continue;
            }
            // Check overlap: lanes that share any portion of their path
            if a_min < lane.to_max && a_max > lane.from_min {
                lane.indices.push(i);
                lane.from_min = lane.from_min.min(a_min);
                lane.to_max = lane.to_max.max(a_max);
                merged = true;
                break;
            }
        }

        if !merged {
            lanes.push(LaneGroup {
                indices: vec![i],
                is_horizontal,
                fixed_coord,
                from_min: a_min,
                to_max: a_max,
            });
        }
    }

    // Only re-route lanes that have 2+ edges
    for lane in &lanes {
        if lane.indices.len() < 2 {
            continue;
        }

        // Sort indices by the along-track start coordinate so lane 0 is
        // the leftmost/topmost edge, lane 1 the next, etc.
        let mut sorted: Vec<(usize, i32)> = lane
            .indices
            .iter()
            .map(|&idx| {
                let e = &diagram.edges[idx];
                let start = e.route[0];
                (idx, if lane.is_horizontal { start.x } else { start.y })
            })
            .collect();
        sorted.sort_by_key(|&(_, start)| start);

        let half_span = ((sorted.len() as i32 - 1) as f64 * LANE_SPACING as f64 / 2.0).round() as i32;

        for (lane_i, &(edge_idx, _)) in sorted.iter().enumerate() {
            let edge = &diagram.edges[edge_idx];
            if edge.route.len() < 2 {
                continue;
            }

            // Offset is centred on the original line: lane 0 goes -half_span,
            // lane 1 goes -half_span + spacing, etc.
            let offset = (lane_i as i32) * LANE_SPACING - half_span;
            if offset == 0 {
                continue; // this lane keeps the original route
            }

            // Rebuild the route: shift the entire straight segment by offset,
            // then add 45° diagonal connectors at both ends.
            let mut new_route: Vec<Point> = Vec::new();

            // Source connector: keep the original first point (node boundary),
            // then walk 45° from the boundary to the offset lane.
            let abs_offset = offset.abs();
            let offset_sign = offset.signum();
            let first = edge.route[0];
            new_route.push(first);

            if abs_offset > 0 {
                // Determine direction away from source for the along-track component.
                // For horizontal edges, this is the x-direction (source → target).
                // For vertical edges, this is the y-direction.
                let route_dir = if edge.route.len() > 1 {
                    if lane.is_horizontal {
                        (edge.route[1].x - edge.route[0].x).signum()
                    } else {
                        (edge.route[1].y - edge.route[0].y).signum()
                    }
                } else {
                    1
                };

                for s in 1..=abs_offset {
                    let step_x = if lane.is_horizontal {
                        // Back up from the source port by s, then go up/down
                        first.x - s * route_dir
                    } else {
                        first.x + s * offset_sign
                    };
                    let step_y = if lane.is_horizontal {
                        first.y + s * offset_sign
                    } else {
                        first.y - s * route_dir
                    };
                    new_route.push(Point::new(step_x, step_y));
                }
            }

            // Middle: shifted straight segment (skip first and last points of original)
            if edge.route.len() > 2 {
                for p in &edge.route[1..edge.route.len() - 1] {
                    if lane.is_horizontal {
                        new_route.push(Point::new(p.x, p.y + offset));
                    } else {
                        new_route.push(Point::new(p.x + offset, p.y));
                    }
                }
            }

            // Target connector: from the last shifted intermediate point back
            // to the original target endpoint at a 45° angle.
            let last = edge.route[edge.route.len() - 1];
            let pre_last = if edge.route.len() >= 2 {
                edge.route[edge.route.len() - 2]
            } else {
                last
            };

            if abs_offset > 0 {
                // The last shifted intermediate point
                let shifted_pre_last = if lane.is_horizontal {
                    Point::new(pre_last.x, pre_last.y + offset)
                } else {
                    Point::new(pre_last.x + offset, pre_last.y)
                };
                // The connector steps from shifted_pre_last to last in exact 45° steps.
                // Step direction: one unit in the route direction, one unit toward the original lane.
                let (dx, dy) = if lane.is_horizontal {
                    // Horizontal edge: route runs in x, offset is in y
                    let route_dir = (last.x - pre_last.x).signum();
                    // Clamp: if route_dir is 0 (both same x), use 1
                    let rd = if route_dir == 0 { 1 } else { route_dir };
                    (rd, offset_sign)
                } else {
                    // Vertical edge: route runs in y, offset is in x
                    let route_dir = (last.y - pre_last.y).signum();
                    let rd = if route_dir == 0 { 1 } else { route_dir };
                    (offset_sign, rd)
                };

                for s in 1..=abs_offset {
                    let step_x = shifted_pre_last.x + s * dx;
                    let step_y = shifted_pre_last.y + s * dy;
                    new_route.push(Point::new(step_x, step_y));
                }
            }

            new_route.push(last);

            let edge_mut = &mut diagram.edges[edge_idx];
            edge_mut.route = new_route;
        }
    }
}
