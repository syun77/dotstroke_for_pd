use crate::model::{Document, VectorObject};
use eframe::egui::{Pos2, Vec2};

pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: 100,
        }
    }
}
impl History {
    pub fn save(&mut self, document: &Document) {
        self.undo.push(document.clone());
        self.redo.clear();
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }
    pub fn undo(&mut self, current: &Document) -> Option<Document> {
        let previous = self.undo.pop()?;
        self.redo.push(current.clone());
        Some(previous)
    }
    pub fn redo(&mut self, current: &Document) -> Option<Document> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        Some(next)
    }
}

pub fn distance_to_segment(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let delta = end - start;
    let length_squared = delta.length_sq();
    if length_squared == 0.0 {
        return point.distance(start);
    }
    let t = ((point - start).dot(delta) / length_squared).clamp(0.0, 1.0);
    point.distance(start + delta * t)
}

pub fn point_in_polygon(point: Pos2, points: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut previous = points.last().copied().unwrap_or([0.0, 0.0]);
    for current in points {
        let crosses = ((current[1] > point.y) != (previous[1] > point.y))
            && (point.x
                < (previous[0] - current[0]) * (point.y - current[1]) / (previous[1] - current[1])
                    + current[0]);
        if crosses {
            inside = !inside;
        }
        previous = *current;
    }
    inside
}

pub fn move_object(object: &mut VectorObject, delta: Vec2) {
    for point in &mut object.points {
        point[0] += delta.x;
        point[1] += delta.y;
    }
    for child in &mut object.children {
        move_object(child, delta);
    }
}
