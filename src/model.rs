use serde::{Deserialize, Serialize};

pub const DEFAULT_WIDTH: i32 = 32;
pub const DEFAULT_HEIGHT: i32 = 32;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub color: String,
    pub blend: String,
    pub width: i32,
    pub cap: String,
    pub fill: bool,
    pub radius: i32,
    pub dither_pattern: String,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: "black".into(),
            blend: "normal".into(),
            width: 1,
            cap: "butt".into(),
            fill: false,
            radius: 4,
            dither_pattern: "none".into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VectorObject {
    #[serde(rename = "type")]
    pub kind: String,
    pub points: Vec<[f32; 2]>,
    pub closed: bool,
    pub style: Style,
    pub transform: serde_json::Value,
    pub children: Vec<VectorObject>,
    pub visible: bool,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Layer {
    pub id: String,
    pub visible: bool,
    pub objects: Vec<VectorObject>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Target {
    pub sdk: String,
    pub width: i32,
    pub height: i32,
    pub coordinate_system: String,
    pub pixel_snap: String,
    pub rounding: String,
    pub clip: bool,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            sdk: "3.1.1".into(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            coordinate_system: "top-left".into(),
            pixel_snap: "integer".into(),
            rounding: "nearest".into(),
            clip: true,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub format: String,
    pub version: i32,
    pub target: Target,
    pub canvas: serde_json::Value,
    pub optimize: serde_json::Value,
    pub layers: Vec<Layer>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            format: "pdvector".into(),
            version: 1,
            target: Target::default(),
            canvas: serde_json::json!({"background":"white", "ditherAnchor":"screen"}),
            optimize: serde_json::json!({"mergeCollinearLines":true, "removeDuplicatePoints":true, "simplifyTolerance":0}),
            layers: vec![Layer {
                id: "layer1".into(),
                visible: true,
                objects: vec![],
            }],
        }
    }
}
