//! Parallel lane assignment — a post-routing pass that fans out forward
//! edges that share a straight segment so they don't draw on top of
//! each other.
//!
//! ## TTB Orientation
//!
//! In Top-to-Bottom layout, forward edges run vertically (South→North)
//! through the layer gutters. When several forward edges share the same
//! X column (e.g. multiple parallel vertical transitions between the
//! same layers), this pass groups them and offsets each one horizontally
//! by one grid cell, with 45° diagonal connectors bridging from the
//! original node boundary to the offset lane.
//!
//! Back-edges in TTB exit via East/West ports and travel up the vertical
//! flanks. Their long runs are horizontal (along the top/bottom of a
//! diagram row) — the lane spreader fans them out vertically.
//!
//! Non-straight (multi-segment) edges whose route contains a long
//! vertical run through a LAYER_GUTTER are also spread apart so multiple
//! layer-skipping edges don't overlap.

use crate::ast::{Diagram, Point};

/// Grid dimension for lane spacing — one grid cell.
const LANE_SPACING: i32 = 10;

/// After all edges are routed, assign lane offsets to parallel forward
/// edges that share the same straight segment, spread cyclic back-edges
/// that share the same horizontal channel, and also fan out coincident
/// vertical gutter runs (layer-skipping and TTB gutter edges).
pub(crate) fn assign_parallel_lanes(diagram: &mut Diagram) {
    assign_forward_lanes(diagram);
    assign_gutter_lanes(diagram);
    assign_back_edge_lanes(diagram);
}

/// Group forward edges by their primary straight segment and offset each
/// parallel edge by one grid cell perpendicular to the run.
///
/// In TTB the primary forward edge is vertical (South→North), so the
/// grouping is by X coordinate (fixed_coord = xs[0]) and the offset is
/// horizontal (along X). Horizontal edges (East↔West) within a layer
/// are grouped by Y and offset vertically.
fn assign_forward_lanes(diagram: &mut Diagram) {
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
                let (dx, dy) = if lane.is_horizontal {
                    // Horizontal edge: route runs in x, offset is in y
                    let route_dir = (last.x - pre_last.x).signum();
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

/// Spread coincident vertical runs that run through layer gutters
/// (TTB vertical channel edges). When a forward edge skips multiple
/// layers, A* routes it vertically through the LAYER_GUTTER channels.
/// Multiple such edges can share the same vertical gutter line; this
/// pass fans them out into distinct lanes, offset horizontally, with
/// 45° connectors at each end.
fn assign_gutter_lanes(diagram: &mut Diagram) {
    struct Run {
        edge_idx: usize,
        start: usize,
        end: usize,
        x: i32,
        y_lo: i32,
        y_hi: i32,
    }

    let mut runs: Vec<Run> = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        // Skip edges that are entirely straight — they're already handled
        // by assign_forward_lanes (vanilla forward edges) or
        // assign_back_edge_lanes (cyclic back-edges).
        if edge.route.len() < 3 {
            continue;
        }
        let xs: Vec<i32> = edge.route.iter().map(|p| p.x).collect();
        let ys: Vec<i32> = edge.route.iter().map(|p| p.y).collect();
        let all_same_x = xs.iter().all(|&x| x == xs[0]);
        let all_same_y = ys.iter().all(|&y| y == ys[0]);
        if all_same_x || all_same_y {
            // Fully straight — handled by assign_forward_lanes.
            continue;
        }

        let Some((start, end)) = longest_vertical_run(&edge.route) else {
            continue;
        };
        // Don't touch runs at the very tail or head of the route —
        // there'd be no connector to patch.
        if start == 0 || end + 1 >= edge.route.len() {
            continue;
        }
        let x = edge.route[start].x;
        let y_lo = edge.route[start..=end].iter().map(|p| p.y).min().unwrap();
        let y_hi = edge.route[start..=end].iter().map(|p| p.y).max().unwrap();
        runs.push(Run { edge_idx: i, start, end, x, y_lo, y_hi });
    }

    // Group runs by shared x where the y-ranges overlap *or touch*.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    'outer: for ri in 0..runs.len() {
        for group in groups.iter_mut() {
            if runs[group[0]].x != runs[ri].x {
                continue;
            }
            let touches = group.iter().any(|&gi| {
                runs[ri].y_lo <= runs[gi].y_hi && runs[gi].y_lo <= runs[ri].y_hi
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
        // Stable order: by input edge index.
        let mut sorted: Vec<usize> = group.clone();
        sorted.sort_by_key(|&gi| runs[gi].edge_idx);

        for (lane_k, &gi) in sorted.iter().enumerate() {
            let dx = (lane_k as i32) * LANE_SPACING;
            if dx == 0 {
                continue;
            }
            let (start, end) = (runs[gi].start, runs[gi].end);
            let route = &mut diagram.edges[runs[gi].edge_idx].route;
            shift_vertical_run_east(route, start, end, dx);
        }
    }
}

/// Spread cyclic back-edges that share the same horizontal "flank
/// channel" into distinct lanes. In TTB, back-edges exit via East/West
/// ports and travel up the sides, then horizontally along the top/bottom
/// of a layer row. When multiple back-edges share the same Y coordinate
/// with overlapping X-ranges, this pass fans them out by offsetting
/// subsequent edges vertically.
fn assign_back_edge_lanes(diagram: &mut Diagram) {
    // First pass: spread vertical side channels (back-edges running
    // East→East or West→West on the same X column).
    struct VRun {
        edge_idx: usize,
        start: usize,
        end: usize,
        x: i32,
        y_lo: i32,
        y_hi: i32,
    }

    let mut vruns: Vec<VRun> = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        if !edge.is_cyclic {
            continue;
        }
        // Look for vertical runs (long stretches with same X).
        let Some((start, end)) = longest_vertical_run(&edge.route) else {
            continue;
        };
        if start == 0 || end + 1 >= edge.route.len() {
            continue;
        }
        let x = edge.route[start].x;
        let y_lo = edge.route[start..=end].iter().map(|p| p.y).min().unwrap();
        let y_hi = edge.route[start..=end].iter().map(|p| p.y).max().unwrap();
        vruns.push(VRun { edge_idx: i, start, end, x, y_lo, y_hi });
    }

    // Group vertical runs by shared x.
    let mut vgroups: Vec<Vec<usize>> = Vec::new();
    'outer: for ri in 0..vruns.len() {
        for group in vgroups.iter_mut() {
            if vruns[group[0]].x != vruns[ri].x {
                continue;
            }
            let touches = group.iter().any(|&gi| {
                vruns[ri].y_lo <= vruns[gi].y_hi && vruns[gi].y_lo <= vruns[ri].y_hi
            });
            if touches {
                group.push(ri);
                continue 'outer;
            }
        }
        vgroups.push(vec![ri]);
    }

    for group in &vgroups {
        if group.len() < 2 {
            continue;
        }
        let mut sorted: Vec<usize> = group.clone();
        sorted.sort_by_key(|&gi| vruns[gi].edge_idx);

        for (lane_k, &gi) in sorted.iter().enumerate() {
            let dx = (lane_k as i32) * LANE_SPACING;
            if dx == 0 {
                continue;
            }
            let (start, end) = (vruns[gi].start, vruns[gi].end);
            let route = &mut diagram.edges[vruns[gi].edge_idx].route;
            shift_vertical_run_east(route, start, end, dx);
        }
    }

    // Second pass: spread horizontal runs (the top/bottom horizontal
    // segments connecting back to the target).
    struct HRun {
        edge_idx: usize,
        start: usize,
        end: usize,
        y: i32,
        x_lo: i32,
        x_hi: i32,
    }

    let mut hruns: Vec<HRun> = Vec::new();
    for (i, edge) in diagram.edges.iter().enumerate() {
        if !edge.is_cyclic {
            continue;
        }
        let Some((start, end)) = longest_horizontal_run(&edge.route) else {
            continue;
        };
        if start == 0 || end + 1 >= edge.route.len() {
            continue;
        }
        let y = edge.route[start].y;
        let x_lo = edge.route[start..=end].iter().map(|p| p.x).min().unwrap();
        let x_hi = edge.route[start..=end].iter().map(|p| p.x).max().unwrap();
        hruns.push(HRun { edge_idx: i, start, end, y, x_lo, x_hi });
    }

    // Group horizontal runs by shared y.
    let mut hgroups: Vec<Vec<usize>> = Vec::new();
    'outer: for ri in 0..hruns.len() {
        for group in hgroups.iter_mut() {
            if hruns[group[0]].y != hruns[ri].y {
                continue;
            }
            let touches = group.iter().any(|&gi| {
                hruns[ri].x_lo <= hruns[gi].x_hi && hruns[gi].x_lo <= hruns[ri].x_hi
            });
            if touches {
                group.push(ri);
                continue 'outer;
            }
        }
        hgroups.push(vec![ri]);
    }

    for group in &hgroups {
        if group.len() < 2 {
            continue;
        }
        let mut sorted: Vec<usize> = group.clone();
        sorted.sort_by_key(|&gi| hruns[gi].edge_idx);

        for (lane_k, &gi) in sorted.iter().enumerate() {
            let dy = (lane_k as i32) * LANE_SPACING;
            if dy == 0 {
                continue;
            }
            let (start, end) = (hruns[gi].start, hruns[gi].end);
            let route = &mut diagram.edges[hruns[gi].edge_idx].route;
            shift_horizontal_run_south(route, start, end, dy);
        }
    }
}

/// Find the longest sub-slice of `route` whose points share a common y
/// coordinate. Returns the inclusive `(start, end)` indices, or `None`
/// if no run of at least 3 points exists.
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

/// Find the longest sub-slice of `route` whose points share a common x
/// coordinate (vertical run). Returns the inclusive `(start, end)`
/// indices, or `None` if no run of at least 3 points exists.
fn longest_vertical_run(route: &[Point]) -> Option<(usize, usize)> {
    if route.len() < 3 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let mut run_start = 0;
    for i in 1..=route.len() {
        let end_of_run = i == route.len() || route[i].x != route[run_start].x;
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

/// Shift `route[start..=end]` to a new x `dx` cells further east,
/// inserting axis-aligned connectors at both ends so the polyline
/// stays strictly orthogonal.
fn shift_vertical_run_east(route: &mut Vec<Point>, start: usize, end: usize, dx: i32) {
    if dx == 0 || start == 0 || end + 1 >= route.len() {
        return;
    }
    let pre = route[start - 1];
    let post = route[end + 1];
    let sy = route[start].y;
    let ey = route[end].y;
    let new_x = route[start].x + dx;

    let mut new_route: Vec<Point> = route[..start].to_vec();

    // Connect `pre` to (new_x, sy). If pre already sits at y=sy we just
    // need a longer horizontal leg (no extra point).
    if pre.y != sy && pre.x != new_x {
        new_route.push(Point::new(pre.x, sy));
    }

    // The shifted run itself.
    for i in start..=end {
        new_route.push(Point::new(new_x, route[i].y));
    }

    // Connect (new_x, ey) back to `post`.
    if post.y != ey && post.x != new_x {
        new_route.push(Point::new(post.x, ey));
    }

    new_route.extend_from_slice(&route[end + 1..]);
    *route = new_route;
}

/// Shift `route[start..=end]` to a new y `dy` cells further south,
/// inserting axis-aligned connectors at both ends so the polyline
/// stays strictly orthogonal.
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
    // need a longer vertical leg (no extra point).
    if pre.x != sx && pre.y != new_y {
        new_route.push(Point::new(sx, pre.y));
    }

    // The shifted run itself.
    for i in start..=end {
        new_route.push(Point::new(route[i].x, new_y));
    }

    // Connect (ex, new_y) back to `post`.
    if post.x != ex && post.y != new_y {
        new_route.push(Point::new(ex, post.y));
    }

    new_route.extend_from_slice(&route[end + 1..]);
    *route = new_route;
}
