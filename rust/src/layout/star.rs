//! Star / hub-and-spoke layout — a complementary layout pass that detects
//! hub nodes (nodes with many direct neighbours) and arranges their
//! neighbours radially at compass points around the hub.
//!
//! ## Motivation
//!
//! In standard Top-to-Bottom layout, every child of a node lands in layer 1
//! and is stacked horizontally in declaration order. When a single node has
//! 4+ neighbours this creates an ugly fan-out — all children on the same
//! horizontal line, edges crammed together.
//!
//! This pass detects hubs and overrides the default TTB positions for the
//! hub's neighbours, placing them around the hub at their natural compass
//! positions (N, NE, E, SE, S, SW, W, NW). Edges from the hub to each
//! neighbour then use the corresponding port pair (e.g. hub→N uses
//! North→South), producing a clean radial star.
//!
//! ## Integration
//!
//! Called from `layout_backbone` after standard TTB placement. It
//! overwrites the positions of nodes that are neighbours of a detected
//! hub. All other nodes keep their TTB positions.

use std::collections::{HashMap, HashSet};

use crate::ast::{Diagram, NodeSize, Point, PortDirection};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum number of unique neighbours (incoming + outgoing) to qualify
/// as a hub node and trigger star layout.
const HUB_MIN_NEIGHBOURS: usize = 4;

/// Base radius from the hub's node boundary edge to the spoke's node
/// boundary edge (in pixels). The actual centre-to-centre distance is
/// this + hub_radius + spoke_radius.
const STAR_BOUNDARY_GAP: i32 = 60;

// ---------------------------------------------------------------------------
// Compass directions
// ---------------------------------------------------------------------------

/// Eight cardinal and intercardinal compass directions, ordered clockwise
/// starting from North.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Compass {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Compass {
    /// All eight directions in clockwise order.
    fn all() -> [Compass; 8] {
        [
            Compass::N,
            Compass::NE,
            Compass::E,
            Compass::SE,
            Compass::S,
            Compass::SW,
            Compass::W,
            Compass::NW,
        ]
    }

    /// Unit vector (dx, dy) for this compass direction.
    pub(crate) fn delta(self) -> (i32, i32) {
        match self {
            Compass::N => (0, -1),
            Compass::NE => (1, -1),
            Compass::E => (1, 0),
            Compass::SE => (1, 1),
            Compass::S => (0, 1),
            Compass::SW => (-1, 1),
            Compass::W => (-1, 0),
            Compass::NW => (-1, -1),
        }
    }

    /// Infer a compass direction from a label string by matching the
    /// first letter (or first two for intercardinal).
    pub(crate) fn from_label(label: &str) -> Option<Compass> {
        let upper = label.to_uppercase();
        let chars: Vec<char> = upper.chars().collect();
        match chars.as_slice() {
            [first, second, ..] if *first == 'N' && *second == 'E' => Some(Compass::NE),
            [first, second, ..] if *first == 'N' && *second == 'W' => Some(Compass::NW),
            [first, second, ..] if *first == 'S' && *second == 'E' => Some(Compass::SE),
            [first, second, ..] if *first == 'S' && *second == 'W' => Some(Compass::SW),
            [first, ..] if *first == 'N' => Some(Compass::N),
            [first, ..] if *first == 'S' => Some(Compass::S),
            [first, ..] if *first == 'E' => Some(Compass::E),
            [first, ..] if *first == 'W' => Some(Compass::W),
            _ => None,
        }
    }

    /// Return the PortDirection for the edge that goes *from* the hub
    /// to a spoke at this compass direction. The port direction tells
    /// A* which face of the hub the edge exits from.
    pub(crate) fn hub_port(self) -> PortDirection {
        match self {
            Compass::N | Compass::NE | Compass::NW => PortDirection::North,
            Compass::S | Compass::SE | Compass::SW => PortDirection::South,
            Compass::E => PortDirection::East,
            Compass::W => PortDirection::West,
        }
    }

    /// Return the PortDirection for the edge that goes *into* a spoke
    /// node at this compass direction from the hub.
    pub(crate) fn spoke_port(self) -> PortDirection {
        match self {
            Compass::N | Compass::NW | Compass::NE => PortDirection::South,
            Compass::S | Compass::SE | Compass::SW => PortDirection::North,
            Compass::E => PortDirection::West,
            Compass::W => PortDirection::East,
        }
    }
}

// ---------------------------------------------------------------------------
// Detecting hubs
// ---------------------------------------------------------------------------

/// Information about a detected hub and its spoke nodes.
#[derive(Debug, Clone)]
pub(crate) struct HubStar {
    /// ID of the hub node.
    pub hub_id: String,
    /// IDs of the neighbour (spoke) nodes, in their assigned compass
    /// order.
    pub spokes: Vec<String>,
}

/// Detect hub nodes in the diagram — nodes with many (≥4) unique
/// neighbours. Returns a list of hubs sorted by degree (most connected
/// first). Only one hub per cluster is returned to avoid conflicting
/// star layouts.
pub(crate) fn detect_hubs(diagram: &Diagram) -> Vec<HubStar> {
    let mut neighbour_count: HashMap<&str, HashSet<&str>> = HashMap::new();

    for edge in &diagram.edges {
        let from = edge.from.as_str();
        let to = edge.to.as_str();
        neighbour_count.entry(from).or_default().insert(to);
        neighbour_count.entry(to).or_default().insert(from);
    }

    let candidates: Vec<(String, Vec<String>)> = neighbour_count
        .iter()
        .filter(|(_, neighbours)| neighbours.len() >= HUB_MIN_NEIGHBOURS)
        .map(|(&id, neighbours)| (id.to_string(), neighbours.iter().map(|n| n.to_string()).collect::<Vec<_>>()))
        .collect();

    // Sort by degree (most neighbours first).
    let mut sorted = candidates;
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let mut hubs: Vec<HubStar> = Vec::new();
    let mut claimed: HashSet<String> = HashSet::new();

    for (hub_id, all_neighbours) in &sorted {
        if claimed.contains(hub_id) {
            continue;
        }

        let spokes: Vec<String> = all_neighbours
            .iter()
            .filter(|n| !claimed.contains(n.as_str()) && n.as_str() != hub_id)
            .take(8)
            .cloned()
            .collect();

        if spokes.len() >= HUB_MIN_NEIGHBOURS.min(8) {
            claimed.insert(hub_id.clone());
            for s in &spokes {
                claimed.insert(s.clone());
            }
            hubs.push(HubStar {
                hub_id: hub_id.clone(),
                spokes,
            });
        }
    }

    hubs
}

// ---------------------------------------------------------------------------
// Star layout placement
// ---------------------------------------------------------------------------

/// Place spoke nodes around their hub at compass positions.
///
/// The hub's position is kept as-is (set by TTB layout). Each spoke
/// is moved to a point a fixed distance from the hub in its assigned
/// compass direction.
///
/// Returns the assigned compass direction for each spoke so the
/// routing phase can use the correct port pair.
pub(crate) fn layout_star(
    diagram: &mut Diagram,
    hub_id: &str,
    spokes: &[String],
) -> HashMap<String, Compass> {
    let hub_pos = diagram
        .node(hub_id)
        .and_then(|n| n.position)
        .expect("hub must have a position from TTB layout");

    let hub_size = diagram
        .node(hub_id)
        .and_then(|n| n.node_size)
        .unwrap_or(NodeSize {
            width: 60,
            height: 60,
        });

    let hub_radius = hub_size.half_w().max(hub_size.half_h()) as i32;

    let mut assignments: HashMap<String, Compass> = HashMap::new();
    let compasses = Compass::all();

    // First pass: assign compass positions by heuristic label matching.
    let mut remaining_spokes: Vec<&str> = Vec::new();
    let mut used_compass: Vec<bool> = vec![false; 8];

    for spoke_id in spokes {
        let found = 'search: {
            // Try the node ID first
            if let Some(compass) = Compass::from_label(spoke_id) {
                let ci = compasses.iter().position(|&c| c == compass).unwrap();
                if !used_compass[ci] {
                    used_compass[ci] = true;
                    assignments.insert(spoke_id.clone(), compass);
                    break 'search true;
                }
            }
            // Then try the node's display label
            if let Some(node) = diagram.node(spoke_id) {
                if let Some(compass) = Compass::from_label(&node.label) {
                    let ci = compasses.iter().position(|&c| c == compass).unwrap();
                    if !used_compass[ci] {
                        used_compass[ci] = true;
                        assignments.insert(spoke_id.clone(), compass);
                        break 'search true;
                    }
                }
            }
            false
        };
        if !found {
            remaining_spokes.push(spoke_id.as_str());
        }
    }

    // Second pass: assign remaining spokes to free compass positions in
    // declaration order.
    let free_compasses: Vec<Compass> = compasses
        .iter()
        .enumerate()
        .filter(|(i, _)| !used_compass[*i])
        .map(|(_, &c)| c)
        .collect();

    for (idx, spoke_id) in remaining_spokes.iter().enumerate() {
        if let Some(&compass) = free_compasses.get(idx) {
            assignments.insert(spoke_id.to_string(), compass);
        }
    }

    // Now place each spoke node at its compass position.
    for spoke_id in spokes {
        if let Some(&compass) = assignments.get(spoke_id.as_str()) {
            let (dx, dy) = compass.delta();

            let spoke_size = diagram
                .node(spoke_id)
                .and_then(|n| n.node_size)
                .unwrap_or(NodeSize {
                    width: 60,
                    height: 60,
                });
            let spoke_radius = spoke_size.half_w().max(spoke_size.half_h()) as i32;

            // Centre-to-centre distance = hub_radius + gap + spoke_radius
            let total_radius = hub_radius + STAR_BOUNDARY_GAP + spoke_radius;

            // For orthogonal directions (N,S,E,W) the centre-to-centre
            // distance needs to account only for the perpendicular
            // half-dimension of each node. For diagonal directions (NE,NW,
            // SE,SW) the full radius works well since the nodes project
            // diagonally.
            let effective_radius = match compass {
                Compass::N | Compass::S => {
                    // Only need half-heights, but use full radius for
                    // consistency — the gap is already generous.
                    // On Y axis the effective reach is half of node height.
                    let hub_extent = hub_size.half_h();
                    let spoke_extent = spoke_size.half_h();
                    hub_extent + STAR_BOUNDARY_GAP + spoke_extent
                }
                Compass::E | Compass::W => {
                    let hub_extent = hub_size.half_w();
                    let spoke_extent = spoke_size.half_w();
                    hub_extent + STAR_BOUNDARY_GAP + spoke_extent
                }
                _ => total_radius,
            };

            let nx = hub_pos.x + dx * effective_radius;
            let ny = hub_pos.y + dy * effective_radius;

            if let Some(node) = diagram.node_mut(spoke_id) {
                node.position = Some(Point::new(nx, ny));
                // Assign a unique high layer number so TTB routing knows
                // these aren't standard forward edges.
                node.layer = Some(99);
            }
        }
    }

    assignments
}
