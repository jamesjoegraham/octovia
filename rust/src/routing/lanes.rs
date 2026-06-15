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
/// edges that share the same straight segment **and** spread cyclic
/// back-edges that share the same horizontal "bottom channel" so each
/// one rides its own row instead of stacking onto the previous edge's
/// line.
pub(crate) fn assign_parallel_lanes(diagram: &mut Diagram) {
    assign_forward_lanes(diagram);
    assign_back_edge_lanes(diagram);
}

/// Group forward edges by their primary straight segment (horizontal
/// edges group by y, vertical by x) and offset each parallel edge by
/// one grid cell perpendicular to the run, with 45° diagonal
/// connectors back to the original node boundary.
fn assign_forward_lanes(diagram: &mut Diagram) {
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

/// Spread cyclic back-edges that share the same horizontal "bottom
/// channel" into distinct lanes. Each back-edge's route contains a
/// long horizontal run along the south of the diagram; when two such
/// runs share a y-coordinate and an overlapping (or merely touching)
/// x-range, A* has effectively painted them onto the same line and
/// they read as a single edge. This pass detects those collisions and
/// pushes each subsequent edge an extra grid cell further south,
/// patching the vertical entry/exit connectors to remain orthogonal.
fn assign_back_edge_lanes(diagram: &mut Diagram) {
    struct Run {
        edge_idx: usize,
        start: usize,
        end: usize,
        y: i32,
        x_lo: i32,
        x_hi: i32,
    }

    let mut runs: Vec<Run> = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        if !edge.is_cyclic {
            continue;
        }
        let Some((start, end)) = longest_horizontal_run(&edge.route) else {
            continue;
        };
        // Don't touch a back-edge whose run sits at the very tail or
        // head of the route — there'd be no connector to patch.
        if start == 0 || end + 1 >= edge.route.len() {
            continue;
        }
        let y = edge.route[start].y;
        let x_lo = edge.route[start..=end].iter().map(|p| p.x).min().unwrap();
        let x_hi = edge.route[start..=end].iter().map(|p| p.x).max().unwrap();
        runs.push(Run { edge_idx: i, start, end, y, x_lo, x_hi });
    }

    // Group runs by shared y where the x-ranges overlap *or touch* at
    // an endpoint — touching ranges still render as a single visual
    // line through the shared point.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    'outer: for ri in 0..runs.len() {
        for group in groups.iter_mut() {
            if runs[group[0]].y != runs[ri].y {
                continue;
            }
            let touches = group.iter().any(|&gi| {
                runs[ri].x_lo <= runs[gi].x_hi && runs[gi].x_lo <= runs[ri].x_hi
            });
            if touches {
                group.push(ri);
                continue 'outer;
            }
        }
        groups.push(vec![ri]);
    }

    for group in &groups {
        if group.len() < 2 {
            continue;
        }
        // Stable order: by input edge index. The first edge in a group
        // keeps its original y; each subsequent edge drops one extra
        // lane to the south.
        let mut sorted: Vec<usize> = group.clone();
        sorted.sort_by_key(|&gi| runs[gi].edge_idx);

        for (lane_k, &gi) in sorted.iter().enumerate() {
            let dy = (lane_k as i32) * LANE_SPACING;
            if dy == 0 {
                continue;
            }
            let (start, end) = (runs[gi].start, runs[gi].end);
            let route = &mut diagram.edges[runs[gi].edge_idx].route;
            shift_horizontal_run_south(route, start, end, dy);
        }
    }
}

/// Find the longest sub-slice of `route` whose points share a common y
/// coordinate. Returns the inclusive `(start, end)` indices, or `None`
/// if no run of at least 3 points exists. Three is the minimum length
/// where the run is long enough to be a "channel" rather than a stray
/// kink in an otherwise vertical route.
fn longest_horizontal_run(route: &[Point]) -> Option<(usize, usize)> {
    if route.len() < 3 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let mut run_start = 0;
    for i in 1..=route.len() {
        let end_of_run = i == route.len() || route[i].y != route[run_start].y;
        if end_of_run {
            let len = i - run_start;
            if len >= 3 && best.is_none_or(|(s, e)| len > e - s + 1) {
                best = Some((run_start, i - 1));
            }
            if i < route.len() {
                run_start = i;
            }
        }
    }
    best
}

/// Shift `route[start..=end]` to a new y `dy` cells further south,
/// inserting axis-aligned connectors at both ends so the polyline
/// stays strictly orthogonal even when `route[start - 1]` or
/// `route[end + 1]` doesn't lie directly above the shifted run.
fn shift_horizontal_run_south(route: &mut Vec<Point>, start: usize, end: usize, dy: i32) {
    if dy == 0 || start == 0 || end + 1 >= route.len() {
        return;
    }
    let pre = route[start - 1];
    let post = route[end + 1];
    let sx = route[start].x;
    let ex = route[end].x;
    let new_y = route[start].y + dy;

    let mut new_route: Vec<Point> = route[..start].to_vec();

    // Connect `pre` to (sx, new_y). If pre already sits at x=sx we just
    // need a longer vertical leg (no extra point). Otherwise insert an
    // L-bend corner at (sx, pre.y) so the segment is two clean
    // orthogonal legs.
    if pre.x != sx && pre.y != new_y {
        new_route.push(Point::new(sx, pre.y));
    }

    // The shifted run itself.
    for i in start..=end {
        new_route.push(Point::new(route[i].x, new_y));
    }

    // Connect (ex, new_y) back to `post`. Mirror logic for the post
    // connector.
    if post.x != ex && post.y != new_y {
        new_route.push(Point::new(ex, post.y));
    }

    new_route.extend_from_slice(&route[end + 1..]);
    *route = new_route;
}
