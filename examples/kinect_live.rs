extern crate freenectrs;
extern crate glwinhelp;
extern crate image;

use glwinhelp::{imgwin, VirtualKeyCode};
use std::time::Duration;

use freenectrs::freenect;


macro_rules! ptry {
    ($x: expr) => (
        match $x{
            Ok(x) => x,
            Err(r) => {println!("Error: {}", r); return }
        }
        )
}

#[inline]
fn depth_to_img(data: &[u16]) -> image::RgbaImage {
    image::RgbaImage::from_fn(640, 480, |x, y| {
        let idx = y * 640 + x;
        let val = data[idx as usize];
        // let val = val / 2048;
        // let val = val / 10;
        let val = if val > 600 { val - 600 } else { 0 };
        let val = val / 2;
        let val = if val > 255 { 255 } else { val };
        let val = val as u8;
        image::Rgba([val, val, val, 255])
    })
}

pub fn main() {
    let ctx = ptry!(freenect::FreenectContext::init_with_video_motor());
    let dev_count = ptry!(ctx.num_devices());
    if dev_count == 0 {
        println!("No device connected - abort");
        return;
    } else {
        println!("Found {} devices, use first", dev_count);
    }
    let device = ptry!(ctx.open_device(0));
    ptry!(device.set_depth_mode(freenect::FreenectResolution::Medium,
                                freenect::FreenectDepthFormat::MM));
    ptry!(device.set_video_mode(freenect::FreenectResolution::Medium,
                                freenect::FreenectVideoFormat::Rgb));

    let mut dwin = imgwin::ImgWindow::new("Live Depth");
    let mut vwin = imgwin::ImgWindow::new("Live RGB");

    let dstream = ptry!(device.depth_stream());
    let vstream = ptry!(device.video_stream());
    let mut dimg = image::RgbaImage::new(640, 480);
    let mut vimg = image::RgbaImage::new(640, 480);
    ctx.spawn_process_thread().unwrap();
    let mut inphandler = InputHandler {
        device: &device,
        is_closed: false,
    };
    loop {
        let _ = imgwin::FixWaitTimer::new(Duration::from_millis(1000 / 25));
        if let Ok((data, _ /* timestamp */)) = dstream.receiver.try_recv() {
            dimg = depth_to_img(&*data);
        }
        if let Ok((data, _ /* timestamp */)) = vstream.receiver.try_recv() {
            vimg = image::RgbaImage::from_fn(640, 480, |x, y| {
                let idx = 3 * (y * 640 + x) as usize;
                let (r, g, b) = (data[idx], data[idx + 1], data[idx + 2]);
                image::Rgba([r, g, b, 255])
            });
        }
        dwin.set_img(dimg.clone());
        vwin.set_img(vimg.clone());
        dwin.redraw();
        vwin.redraw();
        dwin.check_for_event(&mut inphandler);
        vwin.check_for_event(&mut inphandler);
        if inphandler.is_closed {
            break;
        }
    }
    ctx.stop_process_thread().unwrap();
}

struct InputHandler<'a, 'b: 'a> {
    device: &'a freenect::FreenectDevice<'a, 'b>,
    is_closed: bool,
}

impl<'a, 'b> imgwin::EventHandler for InputHandler<'a, 'b> {
    fn close_event(&mut self) {
        self.is_closed = true;
    }
    fn key_event(&mut self, inp: Option<VirtualKeyCode>) {
        if let Some(code) = inp {
            match code {
                VirtualKeyCode::Up => {
                    ptry!(self.device.set_tilt_degree(ptry!(self.device.get_tilt_degree()) + 10.0))
                }
                VirtualKeyCode::Down => {
                    ptry!(self.device.set_tilt_degree(ptry!(self.device.get_tilt_degree()) - 10.0))
                }

                VirtualKeyCode::Q => self.is_closed = true,
                _ => (),
            }
        }
    }
}
