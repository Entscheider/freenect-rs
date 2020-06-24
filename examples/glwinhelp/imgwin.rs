use glium;
use glium::index::PrimitiveType;
use glium::Surface;
use image;
use std::time::{Duration, Instant};

use glium::glutin::event::{ElementState, Event, StartCause, VirtualKeyCode, WindowEvent};
use glium::glutin::event_loop::{ControlFlow, EventLoop};
use glium::glutin::window::WindowBuilder;
use glium::glutin::ContextBuilder;
use glium::texture::{CompressedSrgbTexture2d, RawImage2d};
use glium::{implement_vertex, program, uniform};

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

/// An Application creates windows and runs a main loop
pub struct Application {
    main_loop: EventLoop<()>,
}

impl Application {
    pub fn new() -> Application {
        Application {
            main_loop: EventLoop::new(),
        }
    }

    pub fn new_window(&self, title: impl Into<String>) -> ImgWindow {
        ImgWindow::new(title, &self.main_loop)
    }

    /// Execute the main loop without ever returning. Events are delegated to the given `handler`
    /// and `handler.next_frame` is called `fps` times per seconds.
    /// Whenever `handler.should_exit` turns true, the program exit.
    pub fn run<T: MainloopHandler + 'static>(self, mut handler: T, fps: u32) -> ! {
        self.main_loop.run(move |event, _, control_flow| {
            let now = Instant::now();
            match event {
                Event::WindowEvent {
                    event: win_event, ..
                } => match win_event {
                    WindowEvent::CloseRequested => {
                        handler.close_event();
                    }
                    WindowEvent::KeyboardInput { input, .. }
                        if input.state == ElementState::Pressed =>
                    {
                        handler.key_event(input.virtual_keycode)
                    }
                    _ => (),
                },
                Event::NewEvents(StartCause::ResumeTimeReached { .. })
                | Event::NewEvents(StartCause::Init) => handler.next_frame(),
                _ => (),
            }

            if handler.should_exit() {
                *control_flow = ControlFlow::Exit;
                handler.on_exit();
            } else {
                *control_flow =
                    ControlFlow::WaitUntil(now + Duration::from_secs_f32(1f32 / fps as f32));
            }
        });
    }
}

/// Shows a image with help of opengl (glium)
pub struct ImgWindow {
    texture: Option<CompressedSrgbTexture2d>,
    pub facade: glium::Display,
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
}

/// Implement this trait for handling events that occurs in the main loop
/// and control when the main loop exit.
pub trait MainloopHandler {
    /// Get called whenever a window is closed.
    fn close_event(&mut self);
    /// Get called whenever a key is pressed.
    fn key_event(&mut self, inp: Option<VirtualKeyCode>);
    /// Should return true if the main loop should exit.
    /// Get called after every other event.
    fn should_exit(&self) -> bool;
    /// Get called when the next frame should be drawn.
    fn next_frame(&mut self);
    /// Get called before the main loops end
    fn on_exit(&mut self);
}

impl ImgWindow {
    fn new<T: Into<String>>(title: T, main_loop: &EventLoop<()>) -> ImgWindow {
        let wb = WindowBuilder::new().with_title(title.into());
        let cb = ContextBuilder::new().with_vsync(true);
        let display = glium::Display::new(wb, cb, &main_loop).unwrap();

        // vertex for a rect for drawing an image to the whole window
        let vertex_buffer = glium::VertexBuffer::new(
            &display,
            &[
                Vertex {
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
                },
            ],
        )
        .unwrap();
        let index_buffer =
            glium::IndexBuffer::new(&display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
                .unwrap();
        // just enough shader for drawing images
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

    /// Changes the image which should be drawn to this window. Call `redraw` to show this image
    /// to the user.
    pub fn set_img(&mut self, img: image::RgbaImage) {
        let dim = img.dimensions();
        let text = RawImage2d::from_raw_rgba_reversed(&img.into_raw(), dim);
        self.texture = CompressedSrgbTexture2d::new(&self.facade, text).ok();
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
            target
                .draw(
                    &self.vertex_buffer,
                    &self.index_buffer,
                    &self.program,
                    &uniforms,
                    &Default::default(),
                )
                .unwrap();
        }
        target.finish().unwrap();
        // self.facade.swap_buffers().unwrap();
    }
}
