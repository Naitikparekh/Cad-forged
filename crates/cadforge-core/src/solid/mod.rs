//! 3D solids: triangle meshes with an optional linked extrusion history (port of `src/solid`).
pub mod boolean;
pub mod ext;
pub mod mesh;

use crate::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl V3 {
    pub const fn new(x: f64, y: f64, z: f64) -> V3 {
        V3 { x, y, z }
    }
    pub fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn scale(self, s: f64) -> V3 {
        V3::new(self.x * s, self.y * s, self.z * s)
    }
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        V3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn unit(self) -> V3 {
        let l = self.len();
        if l < 1e-15 {
            V3::new(0.0, 0.0, 1.0)
        } else {
            self.scale(1.0 / l)
        }
    }
}

/// A plain triangle mesh (no identity or styling).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<V3>,
    pub faces: Vec<[u32; 3]>,
}

/// One feature of an associative extrusion history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    /// `new`, `add` or `subtract`.
    pub op: String,
    /// Ids of the source outline entities.
    pub profile: Vec<u64>,
    pub height: f64,
    #[serde(default, rename = "startZ")]
    pub start_z: f64,
    #[serde(default)]
    pub taper: f64,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Program {
    pub features: Vec<Feature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Mesh>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Solid {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    pub vertices: Vec<V3>,
    pub faces: Vec<[u32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<Program>,
}

impl Solid {
    pub fn from_mesh(name: &str, m: Mesh) -> Solid {
        Solid { id: 0, name: name.into(), layer: String::new(), color: None, vertices: m.vertices, faces: m.faces, program: None }
    }
    pub fn mesh(&self) -> Mesh {
        Mesh { vertices: self.vertices.clone(), faces: self.faces.clone() }
    }
}
