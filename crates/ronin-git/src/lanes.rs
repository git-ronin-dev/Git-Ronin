//! Assigns commits to graph columns ("lanes") and computes the line segments
//! for each row, streaming: each commit is placed as it arrives from a
//! topologically ordered walk (children before parents).

use serde::Serialize;
use ts_rs::TS;

/// A line segment within one row. Rows are drawn independently, so each edge
/// spans at most one row: top edge to the node, node to bottom edge, or
/// straight through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Edge {
    pub kind: EdgeKind,
    pub from: u16,
    pub to: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EdgeKind {
    /// Lane `from` passes this row untouched (`from == to`).
    Pass,
    /// Lane `from` enters at the top and ends in the node at column `to`.
    In,
    /// Leaves the node at column `from` towards lane `to` at the bottom.
    Out,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Placement {
    pub lane: u16,
    pub edges: Vec<Edge>,
}

/// Each slot holds the commit that lane is waiting for.
pub struct LaneLayout<Id> {
    lanes: Vec<Option<Id>>,
    max_lanes: u16,
}

impl<Id: PartialEq + Clone> LaneLayout<Id> {
    pub fn new() -> Self {
        Self {
            lanes: Vec::new(),
            max_lanes: 0,
        }
    }

    /// Widest the graph has been so far.
    pub fn max_lanes(&self) -> u16 {
        self.max_lanes
    }

    pub fn place(&mut self, id: &Id, parents: &[Id]) -> Placement {
        let incoming: Vec<usize> = self.lanes_expecting(id).collect();
        let lane = match incoming.first() {
            Some(&first) => first,
            None => self.free_slot(),
        };

        let mut edges = Vec::new();
        for (i, slot) in self.lanes.iter().enumerate() {
            if slot.is_none() {
                continue;
            }
            let kind = if incoming.contains(&i) {
                EdgeKind::In
            } else {
                EdgeKind::Pass
            };
            let to = if kind == EdgeKind::In { lane } else { i };
            edges.push(edge(kind, i, to));
        }
        for &i in &incoming {
            self.lanes[i] = None;
        }

        if let Some((first, rest)) = parents.split_first() {
            // The first parent continues this commit's lane.
            self.lanes[lane] = Some(first.clone());
            edges.push(edge(EdgeKind::Out, lane, lane));
            for parent in rest {
                let existing = self.lanes_expecting(parent).next();
                let target = match existing {
                    Some(j) => j,
                    None => {
                        let j = self.free_slot();
                        self.lanes[j] = Some(parent.clone());
                        j
                    }
                };
                edges.push(edge(EdgeKind::Out, lane, target));
            }
        }

        while self.lanes.last().is_some_and(Option::is_none) {
            self.lanes.pop();
        }
        self.max_lanes = self
            .max_lanes
            .max(self.lanes.len() as u16)
            .max(lane as u16 + 1);
        Placement {
            lane: lane as u16,
            edges,
        }
    }

    fn lanes_expecting<'a>(&'a self, id: &'a Id) -> impl Iterator<Item = usize> + 'a {
        self.lanes
            .iter()
            .enumerate()
            .filter(move |(_, slot)| slot.as_ref() == Some(id))
            .map(|(i, _)| i)
    }

    /// Reuses the leftmost empty lane, or opens a new one on the right.
    fn free_slot(&mut self) -> usize {
        match self.lanes.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }
}

fn edge(kind: EdgeKind, from: usize, to: usize) -> Edge {
    Edge {
        kind,
        from: from as u16,
        to: to as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EdgeKind::*;

    type Row = (u16, Vec<(EdgeKind, u16, u16)>);

    /// Places `(id, parents)` rows and returns `(lane, edges)` per row.
    fn layout(rows: &[(char, &str)]) -> Vec<Row> {
        let mut l = LaneLayout::new();
        rows.iter()
            .map(|(id, parents)| {
                let parents: Vec<char> = parents.chars().collect();
                let p = l.place(id, &parents);
                let edges = p.edges.iter().map(|e| (e.kind, e.from, e.to)).collect();
                (p.lane, edges)
            })
            .collect()
    }

    #[test]
    fn linear_history_stays_in_one_lane() {
        assert_eq!(
            layout(&[('c', "b"), ('b', "a"), ('a', "")]),
            [
                (0, vec![(Out, 0, 0)]),
                (0, vec![(In, 0, 0), (Out, 0, 0)]),
                (0, vec![(In, 0, 0)]),
            ]
        );
    }

    #[test]
    fn branch_and_merge() {
        //  m        merge of b into main
        //  |\
        //  | b
        //  c |
        //  |/
        //  a
        assert_eq!(
            layout(&[('m', "cb"), ('b', "a"), ('c', "a"), ('a', "")]),
            [
                (0, vec![(Out, 0, 0), (Out, 0, 1)]),
                (1, vec![(Pass, 0, 0), (In, 1, 1), (Out, 1, 1)]),
                (0, vec![(In, 0, 0), (Pass, 1, 1), (Out, 0, 0)]),
                (0, vec![(In, 0, 0), (In, 1, 0)]),
            ]
        );
    }

    #[test]
    fn two_tips_converge_on_shared_parent() {
        // Two branch heads x and y, both children of a.
        assert_eq!(
            layout(&[('x', "a"), ('y', "a"), ('a', "")]),
            [
                (0, vec![(Out, 0, 0)]),
                (1, vec![(Pass, 0, 0), (Out, 1, 1)]),
                (0, vec![(In, 0, 0), (In, 1, 0)]),
            ]
        );
    }

    #[test]
    fn freed_lanes_are_reused() {
        // x ends in its own root, then y starts: y takes the freed lane 1.
        let rows = layout(&[('m', "a"), ('x', ""), ('y', "a"), ('a', "")]);
        assert_eq!(rows[1].0, 1);
        assert_eq!(rows[2].0, 1);
    }

    #[test]
    fn octopus_merge_opens_a_lane_per_parent() {
        let rows = layout(&[('o', "abc"), ('c', ""), ('b', ""), ('a', "")]);
        assert_eq!(rows[0], (0, vec![(Out, 0, 0), (Out, 0, 1), (Out, 0, 2)]));
    }

    #[test]
    fn merge_into_existing_lane_reuses_it() {
        // m merges b, which lane 1 already waits for because of y.
        let rows = layout(&[('y', "b"), ('m', "ab"), ('b', "a"), ('a', "")]);
        assert_eq!(rows[1], (1, vec![(Pass, 0, 0), (Out, 1, 1), (Out, 1, 0)]));
    }

    #[test]
    fn tracks_max_width() {
        let mut l = LaneLayout::new();
        l.place(&'o', &['a', 'b', 'c']);
        l.place(&'c', &[]);
        assert_eq!(l.max_lanes(), 3);
    }
}
