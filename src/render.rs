use eframe::egui::{Pos2, Rect, Vec2};

#[derive(Clone, Copy)]
pub struct ViewTransform {
    pub zoom: f32,
    pub pan: Vec2,
}
impl ViewTransform {
    pub fn screen_to_document(self, rect: Rect, position: Pos2) -> Pos2 {
        Pos2::new(
            (position.x - rect.left() - self.pan.x) / self.zoom,
            (position.y - rect.top() - self.pan.y) / self.zoom,
        )
    }
    pub fn document_to_screen(self, rect: Rect, point: [f32; 2]) -> Pos2 {
        Pos2::new(
            rect.left() + self.pan.x + point[0] * self.zoom,
            rect.top() + self.pan.y + point[1] * self.zoom,
        )
    }
    pub fn max_zoom(viewport: Vec2) -> f32 {
        (viewport.x.min(viewport.y) / 8.0).max(0.25)
    }
}
