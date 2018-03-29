use image;
use glium;
use glium::{DisplayBuild, Surface};
use glium::backend::glutin_backend::GlutinFacade;
use glium::index::PrimitiveType;
use glium::glutin::{Event, VirtualKeyCode};
use glium::glutin;

#[derive(Copy,Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

/// Shows a image with help of opengl (glium)
pub struct ImgWindow {
    texture: Option<glium::texture::CompressedSrgbTexture2d>,
    pub facade: GlutinFacade,
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
}

pub trait EventHandler {
    fn close_event(&mut self);
    fn key_event(&mut self, inp: Option<VirtualKeyCode>);
}

impl ImgWindow {
    pub fn new<T: Into<String>>(title: T) -> ImgWindow {
        // let display = glium::glutin::WindowBuilder::new().with_vsync().with_title(title.into()).build_glium().unwrap();
        let display =
            glium::glutin::WindowBuilder::new().with_title(title.into()).build_glium().unwrap();
        let vertex_buffer = glium::VertexBuffer::new(&display,
                                                     &[Vertex {
                                                           position: [-1.0, -1.0],
                                                           tex_coords: [0.0, 0.0],
                                                       },
                                                       Vertex {
                                                           position: [-1.0, 1.0],
                                                           tex_coords: [0.0, 1.0],
                                                       },
                                                       Vertex {
                                                           position: [1.0, 1.0],
                                                           tex_coords: [1.0, 1.0],
                                                       },
                                                       Vertex {
                                                           position: [1.0, -1.0],
                                                           tex_coords: [1.0, 0.0],
                                                       }])
            .unwrap();
        let index_buffer =
            glium::IndexBuffer::new(&display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
                .unwrap();
        let program = program!(&display, 
        140 => {
            vertex: "
            #version 140

            uniform lowp mat4 matrix;
            in vec2 position;
            in vec2 tex_coords;
            out vec2 v_tex_coords;
            void main(){
                gl_Position = matrix * vec4(position, 0.0, 1.0);
                v_tex_coords = tex_coords;
            }
        ",
        fragment: "
            #version 140

            uniform sampler2D tex;
            in vec2 v_tex_coords;
            out vec4 f_color;
            void main(){
                f_color = texture(tex, v_tex_coords);
            }
        "
        },)
            .unwrap();
        ImgWindow {
            texture: None,
            facade: display,
            vertex_buffer: vertex_buffer,
            index_buffer: index_buffer,
            program: program,
        }
    }
    pub fn set_img(&mut self, img: image::RgbaImage) {
        let dim = img.dimensions();
        let text = glium::texture::RawImage2d::from_raw_rgba_reversed(img.into_raw(), dim);
        self.texture = glium::texture::CompressedSrgbTexture2d::new(&self.facade, text).ok();

    }

    /// Redraws using opengl
    pub fn redraw(&self) {
        let mut target = self.facade.draw();
        target.clear_color(0.0, 0.0, 0.0, 0.0);
        if let Some(ref texture) = self.texture {
            let uniforms = uniform! {
                matrix: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0f32]
                ],
                tex: texture
            };
            target.draw(&self.vertex_buffer,
                      &self.index_buffer,
                      &self.program,
                      &uniforms,
                      &Default::default())
                .unwrap();
        }
        target.finish().unwrap();
        // self.facade.swap_buffers().unwrap();
    }

    /// Checks if there were any close events
    pub fn check_for_close(&self) -> bool {
        for event in self.facade.poll_events() {
            match event {
                glium::glutin::Event::Closed => return true,
                _ => (),
            }
        }
        false
    }

    pub fn check_for_event<T: EventHandler>(&self, handler: &mut T) {
        for event in self.facade.poll_events() {
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
}

use std::time::{Instant, Duration};
use std::thread;

/// Saves the time when created and let the current thread
/// sleep on drop such that at least duration time has passed
/// from creation until drop returns.
pub struct FixWaitTimer {
    begin: Instant,
    duration: Duration,
}

impl FixWaitTimer {
    pub fn new(duration: Duration) -> FixWaitTimer {
        FixWaitTimer {
            begin: Instant::now(),
            duration: duration,
        }
    }
}

impl Drop for FixWaitTimer {
    fn drop(&mut self) {
        let passed_time = Instant::now() - self.begin;
        if passed_time < self.duration {
            thread::sleep(self.duration - passed_time);
        }
    }
}
