// Function copied from here https://github.com/tomaka/glium/blob/master/examples/support/mod.rs
use obj;
use genmesh;

#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub texture: [f32; 2],
    pub id: u32,
}
implement_vertex!(Vertex, position, normal, texture, id);

// Returns a vertex buffer that should be rendered as `TrianglesList`.
pub fn load_wavefront(data: &[u8], id: u32) -> Vec<Vertex> {

    let mut data = ::std::io::BufReader::new(data);
    let data = obj::Obj::load(&mut data);

    let mut vertex_data = Vec::new();

    for object in data.object_iter() {
        for shape in object.group_iter().flat_map(|g| g.indices().iter()) {
            match shape {
                &genmesh::Polygon::PolyTri(genmesh::Triangle { x: v1, y: v2, z: v3 }) => {
                    for v in [v1, v2, v3].iter() {
                        let position = data.position()[v.0];
                        let texture = v.1.map(|index| data.texture()[index]);
                        let normal = v.2.map(|index| data.normal()[index]);

                        let texture = texture.unwrap_or([0.0, 0.0]);
                        let normal = normal.unwrap_or([0.0, 0.0, 0.0]);

                        vertex_data.push(Vertex {
                            position: position,
                            normal: normal,
                            texture: texture,
                            id: id,
                        })
                    }
                }
                _ => unimplemented!(),
            }
        }
    }

    vertex_data
}
