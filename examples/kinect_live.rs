mod glwinhelp;

use glwinhelp::{imgwin, VirtualKeyCode};

use freenectrs::freenect;
use std::error::Error;
use freenectrs::freenect::{FreenectError, FreenectVideoStream, FreenectDepthStream, FreenectContext};
use crate::glwinhelp::imgwin::ImgWindow;

#[inline]
fn depth_to_img(data: &[u16]) -> image::RgbaImage {
    image::RgbaImage::from_fn(640, 480, |x, y| {
        let idx = y * 640 + x;
        // the depth value for the current pixel
        let depth_value = data[idx as usize];

        // we start at value of 600 for depth
        let depth_value = if depth_value > 600 { depth_value - 600 } else { 0 };
        // scale the value down
        let depth_value = depth_value / 2;
        // and use this value as a gray value by clipping everything above the maximal
        // allowed value 255.
        let gray_value = if depth_value > 255 { 255 } else { depth_value };
        let gray_value = gray_value as u8;
        image::Rgba([gray_value, gray_value, gray_value, 255])
    })
}

pub fn main() -> Result<(), Box<dyn Error>>{

    // we init the device with support for depth, video and motor
    let ctx = Box::new(freenect::FreenectContext::init_with_video_motor()?);
    // We create a 'static lifetime by leaking. Doing so we can fulfill the requirement of glium's
    // mainloop run (called in Application::run)
    let ctx = Box::leak(ctx);

    // check if a kinect device is available
    let dev_count = ctx.num_devices()?;
    if dev_count == 0 {
        eprintln!("No device connected - abort");
        return Ok(());
    } else {
        println!("Found {} devices, use first", dev_count);
    }
    // For simplification we always take the fist available device
    let device = Box::new(ctx.open_device(0)?);
    // Same as above.
    // We create a 'static lifetime by leaking. Doing so we can fulfill the requirement of glium's
    // mainloop run (called in Application::run)
    let device = Box::leak(device);
    // init the depth and video mode
    device.set_depth_mode(freenect::FreenectResolution::Medium,
                         freenect::FreenectDepthFormat::MM)?;
    device.set_video_mode(freenect::FreenectResolution::Medium,
                         freenect::FreenectVideoFormat::Rgb)?;

    // creating the gui elements
    let app = imgwin::Application::new();

    let dwin = app.new_window("Live Depth");
    let vwin = app.new_window("Live RGB");

    // getting the streams
    let dstream = device.depth_stream()?;
    let vstream = device.video_stream()?;

    // the image on which we draw the rgb and depth information
    let dimg = image::RgbaImage::new(640, 480);
    let vimg = image::RgbaImage::new(640, 480);

    // run freenects main loop
    ctx.spawn_process_thread().unwrap();

    let input_handler = InputHandler {
        device,
        is_closed: false,
        vstream,
        dstream,
        dimg,
        vimg,
        dwin,
        vwin,
        ctx
    };
    // run the gui main loop
    app.run(input_handler, 25);
}

/// Handler for the main loop
struct InputHandler<'a, 'b: 'a> {
    /// freenect device we actually use
    device: &'a freenect::FreenectDevice<'a, 'b>,
    /// indicates if a windows get close (-> exit app)
    is_closed: bool,
    /// the rgb bytes stream from kinect
    vstream: FreenectVideoStream<'a, 'b>,
    /// the depth bytes from kinect
    dstream: FreenectDepthStream<'a, 'b>,
    /// the image we create from the depth bytes
    dimg: image::RgbaImage,
    /// the image we create from the rgb bytes
    vimg: image::RgbaImage,
    /// the window on which we draw the depth image
    dwin: ImgWindow,
    /// the window on which we draw the rgb image
    vwin: ImgWindow,
    /// the freenect context so we can stop its main loop
    ctx: &'a FreenectContext
}

impl<'a, 'b> imgwin::MainloopHandler for InputHandler<'a, 'b> {

    fn close_event(&mut self) {
        self.is_closed = true;
    }

    fn key_event(&mut self, inp: Option<VirtualKeyCode>) {
        let mut inner = || -> Result<(), FreenectError> {
            if let Some(code) = inp {
                match code {
                    VirtualKeyCode::Up => {
                        // move the kinect up
                        let tilt_degree = self.device.get_tilt_degree()?;
                        self.device.set_tilt_degree(tilt_degree + 10.0)?;
                    }
                    VirtualKeyCode::Down => {
                        // move the kinect down
                        let tilt_degree = self.device.get_tilt_degree()?;
                        self.device.set_tilt_degree(tilt_degree - 10.0)?;
                    }

                    VirtualKeyCode::Q => self.is_closed = true,
                    _ => (),
                }
            }
            Ok(())
        };
        inner().unwrap_or_else(|err| eprintln!("Error: {}", err));
    }

    fn should_exit(&self) -> bool {
        self.is_closed
    }

    fn next_frame(&mut self) {
        // get and render the depth bytes to a image
        if let Ok((data, _ /* timestamp */)) = self.dstream.receiver.try_recv() {
            self.dimg = depth_to_img(&*data);
        }

        // get and create an image from the rgb byzes
        if let Ok((data, _ /* timestamp */)) = self.vstream.receiver.try_recv() {
            self.vimg = image::RgbaImage::from_fn(640, 480, |x, y| {
                let idx = 3 * (y * 640 + x) as usize;
                let (r, g, b) = (data[idx], data[idx + 1], data[idx + 2]);
                image::Rgba([r, g, b, 255])
            });
        }

        // and draw it to window
        let dimg = self.dimg.clone();
        self.dwin.set_img(dimg);
        let vimg = self.vimg.clone();
        self.vwin.set_img(vimg);
        self.dwin.redraw();
        self.vwin.redraw();
    }

    fn on_exit(&mut self) {
        // stop the freenect main loop thread on exit
        self.ctx.stop_process_thread().unwrap();
    }
}
