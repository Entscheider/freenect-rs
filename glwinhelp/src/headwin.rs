
use super::loadobj::{Vertex, load_wavefront};
use glium;
use glium::glutin;
use glium::{DisplayBuild, Surface};
use glium::backend::glutin_backend::GlutinFacade;
use glium::glutin::Event;
use image;
use super::imgwin::EventHandler;


/// A window which shows a image and the model.obj 3d object at the same time.
pub struct HeadWin {
    texture: Option<glium::texture::CompressedSrgbTexture2d>,
    pub display: GlutinFacade,
    vertex_buffer: glium::VertexBuffer<Vertex>,
    program: glium::Program,
    rot: [f32; 3],
    pos: [f32; 3],
    scale: f32,
    intrinsic: [[f32; 3]; 3],
}

impl HeadWin {
    /// Creates a HeadWin using the given `title` and the given `intrinsic` matrix for showing
    /// the 3d object.
    pub fn new<T: Into<String>>(title: T, intrinsic: [[f32; 3]; 3]) -> HeadWin {
        let display = glium::glutin::WindowBuilder::new()
            .with_title(title.into())
            .with_depth_buffer(24)
            .build_glium()
            .unwrap();
        let mut vertex_data = load_wavefront(include_bytes!("model.obj"), 0u32);
        vertex_data.append(&mut imagevertex(1u32));
        vertex_data.append(&mut midpointvertex(2u32));
        let vertex_buffer = glium::vertex::VertexBuffer::new(&display, &vertex_data).unwrap();
        let program = program!(&display, 
                               140 => {vertex: VERTEX_SHADER, fragment: FRAGMENT_SHADER})
            .unwrap();
        HeadWin {
            texture: None,
            display: display,
            vertex_buffer: vertex_buffer,
            program: program,
            rot: [0.0, 0.0, 0.0],
            pos: [0.0, 0.0, 0.0],
            scale: 0.0f32,
            intrinsic: intrinsic,
        }
    }

    /// Sets transformation parameter for the 3d object
    pub fn update_transformation(&mut self, rot: [f32; 3], pos: [f32; 3], scale: f32) {
        self.rot = rot;
        self.pos = pos;
        self.scale = scale;
    }

    /// Sets the image which should be shown
    pub fn update_image(&mut self, img: image::RgbaImage) {
        let dim = img.dimensions();
        let text = glium::texture::RawImage2d::from_raw_rgba_reversed(img.into_raw(), dim);
        self.texture = glium::texture::CompressedSrgbTexture2d::new(&self.display, text).ok();
    }

    /// Redraws using opengl
    pub fn redraw(&self) {
        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::DepthTest::IfLess,
                write: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut target = self.display.draw();
        target.clear_color_and_depth((0.0, 0.0, 0.0, 0.0), 1.0);
        if let Some(ref texture) = self.texture {
            let uniforms = uniform!{
                transform:self.transform_matrix(),
                persp_matrix: self.persp_matrix(),
                view_matrix: [
                    [1.0 , 0.0 , 0.0 , 0.0] ,
                    [0.0 , 1.0 , 0.0 , 0.0] ,
                    [0.0 , 0.0 , 1.0 , 0.0] ,
                    [0.0 , 0.0 , 0.0 , 1.0f32] ,
                ],
                rotx: rot_x(-self.rot[2]),
                roty: rot_y(-self.rot[1]),
                rotz: rot_z(self.rot[0]),
                tex: texture,
            };
            target.draw(&self.vertex_buffer,
                      &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                      &self.program,
                      &uniforms,
                      &params)
                .unwrap();
        }
        target.finish().unwrap();
    }

    pub fn check_for_event<T: EventHandler>(&self, handler: &mut T) {
        for event in self.display.poll_events() {
            match event {
                Event::Closed => {
                    handler.close_event();
                    return;
                }
                Event::KeyboardInput(glutin::ElementState::Pressed, _, code) => {
                    handler.key_event(code)
                }
                _ => (),
            }
        }
    }

    fn transform_matrix(&self) -> [[f32; 4]; 4] {
        let (x, y, z) = (self.pos[0], self.pos[1], self.pos[2]);
        let s = self.scale;
        [[s, 0.0, 0.0, 0.0], [0.0, s, 0.0, 0.0], [0.0, 0.0, s, 0.0], [x, -y, z, 1.0f32]]
    }

    /// Sets the intrinsic matrix  `intr` for showing the 3d object
    pub fn set_intrinsic(&mut self, intr: [[f32; 3]; 3]) {
        self.intrinsic = intr;
    }


    fn persp_matrix(&self) -> [[f32; 4]; 4] {
        let mut width = 1.0;
        let mut height = 1.0;
        if let Some(ref img) = self.texture {
            width = img.get_width() as f32;
            height = img.get_height().unwrap_or(1) as f32;
        }

        let mat = &self.intrinsic;
        let fx = mat[0][0];
        let s = mat[0][1];
        let cx = mat[0][2];
        let fy = mat[1][1];
        let cy = mat[1][2];
        // https://stackoverflow.com/questions/22064084/how-to-create-perspective-projection-matrix-given-focal-points-and-camera-princ
        // 2*fx/W,0,0,0,2*s/W,2*fy/H,0,0,2*(cx/W)-1,2*(cy/H)-1,(zmax+zmin)/(zmax-zmin),1,0,0,2*zmax*zmin/(zmin-zmax),0};
        let zmax = 2024.0;
        // let zmin = 500.0;
        let zmin = 300.0;
        [[2.0 * fx / width, 0.0, 0.0, 0.0],
         [2.0 * s / width, 2.0 * fy / height, 0.0, 0.0],
         [2.0 * (cx / width) - 1.0, 2.0 * (cy / height) - 1.0, (zmax + zmin) / (zmax - zmin), 1.0],
         [0.0, 0.0, 2.0 * zmax * zmin / (zmin - zmax), 0.0]]
    }
}


fn rot_x(angle: f32) -> [[f32; 4]; 4] {
    [[1.0, 0.0, 0.0, 0.0],
     [0.0, angle.cos(), angle.sin(), 0.0],
     [0.0, -angle.sin(), angle.cos(), 0.0],
     [0.0, 0.0, 0.0, 1.0]]
}

fn rot_y(angle: f32) -> [[f32; 4]; 4] {
    [[angle.cos(), 0.0, angle.sin(), 0.0],
     [0.0, 1.0, 0.0, 0.0],
     [-angle.sin(), 0.0, angle.cos(), 0.0],
     [0.0, 0.0, 0.0, 1.0]]
}

fn rot_z(angle: f32) -> [[f32; 4]; 4] {
    [[angle.cos(), angle.sin(), 0.0, 0.0],
     [-angle.sin(), angle.cos(), 0.0, 0.0],
     [0.0, 0.0, 1.0, 0.0],
     [0.0, 0.0, 0.0, 1.0]]
}

fn imagevertex(id: u32) -> Vec<Vertex> {
    vec![Vertex {
             position: [-1.0, -1.0, 0.99],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [-1.0, 1.0, 0.99],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 1.0],
             id: id,
         },
         Vertex {
             position: [1.0, 1.0, 0.999],
             normal: [0.0, 0.0, 0.0],
             texture: [1.0, 1.0],
             id: id,
         },

         Vertex {
             position: [-1.0, -1.0, 0.999],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [1.0, 1.0, 0.999],
             normal: [0.0, 0.0, 0.0],
             texture: [1.0, 1.0],
             id: id,
         },
         Vertex {
             position: [1.0, -1.0, 0.999],
             normal: [0.0, 0.0, 0.0],
             texture: [1.0, 0.0],
             id: id,
         }]
}

fn midpointvertex(id: u32) -> Vec<Vertex> {
    vec![Vertex {
             position: [-0.05, -0.05, 0.0],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [-0.05, 0.05, 0.0],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [0.05, 0.05, 0.0],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [-0.05, -0.05, 0.0],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [0.05, 0.05, 0.0],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         },
         Vertex {
             position: [0.05, -0.05, 0.0],
             normal: [0.0, 0.0, 0.0],
             texture: [0.0, 0.0],
             id: id,
         }]
}

static VERTEX_SHADER: &'static str =
    "
    #version 140

    uniform mat4 persp_matrix;
    uniform mat4 view_matrix;
    uniform \
     mat4 transform;
    uniform mat4 rotx;
    uniform mat4 roty;
    uniform mat4 rotz;

    in \
     vec3 position;
    in vec3 normal;
    in vec2 texture;
    in uint id;


    out vec3 \
     v_position;
    out vec3 v_normal;
    out vec2 v_tex_coords;

    flat out uint v_id;

    \
     void main(){
        v_position = position;
        v_normal = normal;
        v_tex_coords \
     = texture;
        v_id = id;
        if (int(id) == 0) {
            gl_Position =  \
     persp_matrix * view_matrix * transform *  rotx * roty * rotz * vec4(v_position * 0.5, 1.0);
        \
     }else if (int(id) == 2){
            gl_Position = persp_matrix  * view_matrix * transform * \
     vec4(100*position, 1.0);
            gl_Position.z = 0.01;
        }else{
            \
     gl_Position = vec4(position, 1.0);
        }
    }
";

static FRAGMENT_SHADER: &'static str = "
    #version 140

    uniform sampler2D tex;
    in vec3 v_normal;
    in vec2 v_tex_coords;
    flat in uint v_id;
    out vec4 f_color;

    const vec3 LIGHT = vec3(-0.2, 0.8, 0.1);
    
    void main(){
        if (int(v_id) == 1){
            f_color = texture(tex, v_tex_coords);
        }else if (int(v_id) == 2){
            f_color = vec4(0.0,1.0,1.0,1.0);
        }else{
            lowp float lum = max(dot(normalize(v_normal), normalize(LIGHT)), 0.0);
            lowp vec3 color = (0.3 + 0.7 * lum) * vec3(1.0, 1.0, 1.0);
            f_color = vec4(color, 1.0);
        }
    }
";
