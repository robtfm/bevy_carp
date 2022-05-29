use bevy::{prelude::*, render::mesh::{Indices, PrimitiveTopology}};

#[derive(Debug, Copy, Clone)]
pub struct BLQuad {
    pub size: Vec2,
    pub offset: Vec2,
}

impl BLQuad {
    pub fn new(size: Vec2, offset: Vec2) -> Self {
        Self { size, offset }
    }
}

impl From<BLQuad> for Mesh {
    fn from(quad: BLQuad) -> Self {
        let vertices = [
            ([0.0,          0.0,            0.0], [0.0, 0.0, 1.0], [quad.offset.x,                  quad.offset.y]),
            ([0.0,          quad.size.y,    0.0], [0.0, 0.0, 1.0], [quad.offset.x,                  quad.offset.y + quad.size.y]),
            ([quad.size.x,  quad.size.y,    0.0], [0.0, 0.0, 1.0], [quad.offset.x + quad.size.x,    quad.offset.y + quad.size.y]),
            ([quad.size.x,  0.0,            0.0], [0.0, 0.0, 1.0], [quad.offset.x + quad.size.x,    quad.offset.y]),
        ];

        let indices = Indices::U32(vec![0, 2, 1, 0, 3, 2]);

        let mut positions = Vec::<[f32; 3]>::new();
        let mut normals = Vec::<[f32; 3]>::new();
        let mut uvs = Vec::<[f32; 2]>::new();
        for (position, normal, uv) in &vertices {
            positions.push(*position);
            normals.push(*normal);
            uvs.push(*uv);
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(indices));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh
    }
}