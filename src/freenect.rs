use super::freenect_ffi as ffi;
use std::mem;
use std::ptr;
use std::result;
use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel, SyncSender, sync_channel, Receiver, TryRecvError,
                      TrySendError};
use std::slice;
use std;
use std::error::Error;
use std::fmt;
use std::thread;
use std::cell::RefCell;
use std::mem::MaybeUninit;

#[derive(Debug)]
pub struct FreenectError {
    reason: String,
}

impl FreenectError {
    fn new<T: Into<String>>(text: T) -> FreenectError {
        FreenectError { reason: text.into() }
    }
}
impl fmt::Display for FreenectError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FreenectError: {}", self.reason)
    }
}

impl Error for FreenectError {
    fn description(&self) -> &str {
        &*self.reason
    }
}

pub type Result<T> = result::Result<T, FreenectError>;

/// FreenectContext should be used as the main point to interact with Kinect.
pub struct FreenectContext {
    ctx: *mut ffi::freenect_context,
    drop_sender: Mutex<Option<Sender<()>>>,
    use_video: bool,
    thread_joiner: RefCell<Option<thread::JoinHandle<()>>>,
}

impl FreenectContext {
    /// Initializes the context. Use [`setup_video()`][setup_video] or [`setup_video_motor()`][setup_video_motor] if you want to fetch data or to control motor.
    ///
    /// [setup_video]: struct.FreenectContext.html#method.setup_video
    /// [setup_video_motor]: struct.FreenectContext.html#method.setup_video_motor
    pub fn init() -> Result<FreenectContext> {
        unsafe {
            let mut ctx: *mut ffi::freenect_context = MaybeUninit::uninit().assume_init();
            let res = ffi::freenect_init(&mut ctx, ptr::null_mut());
            if res < 0 {
                return Err(FreenectError::new("Unable to create freenect context"));
            }
            let res = FreenectContext {
                ctx: ctx,
                use_video: false,
                drop_sender: Mutex::new(None),
                thread_joiner: RefCell::new(None),
            };
            Ok(res)
        }
    }

    /// Tells libfreenect to select the camera subdevice
    pub fn setup_video(mut self) -> FreenectContext {
        unsafe {
            ffi::freenect_select_subdevices(self.ctx,
                                            ffi::freenect_device_flags::FREENECT_DEVICE_CAMERA);
            self.use_video = true;
            self
        }
    }

    /// Tells libfreenect to select the camera and motor subdevice
    pub fn setup_video_motor(mut self) -> FreenectContext {
        unsafe {
            // ffi::freenect_select_subdevices(self.ctx,
            //                                 ffi::freenect_device_flags::FREENECT_DEVICE_CAMERA | ffi::freenect_device_flags::FREENECT_DEVICE_MOTOR);
            // TODO Bit-Flags support
            ffi::freenect_select_subdevices(self.ctx, mem::transmute(3));
            self.use_video = true;
            self
        }
    }

    /// Initializes the context directly for fetching rgb and depth data
    pub fn init_with_video() -> Result<FreenectContext> {
        FreenectContext::init().map(|x| x.setup_video())
    }

    /// Initializes the context directly for fetching rgb and depth data and using motor
    pub fn init_with_video_motor() -> Result<FreenectContext> {
        FreenectContext::init().map(|x| x.setup_video_motor())
    }

    /// Returns the number of available devices
    pub fn num_devices(&self) -> Result<u32> {
        unsafe {
            let res = ffi::freenect_num_devices(self.ctx);
            if res < 0 {
                return Err(FreenectError::new("Unable to retrieve number of freenect devices"));
            }
            Ok(res as u32)
        }
    }

    /// Opens a device using the given number.
    pub fn open_device(&self, nr: u32) -> Result<FreenectDevice> {
        if nr >= self.num_devices()? {
            return Err(FreenectError::new(format!("Device nr {} not found", nr)));
        }
        unsafe {
            let mut dev: *mut ffi::freenect_device = MaybeUninit::uninit().assume_init();
            if ffi::freenect_open_device(self.ctx, &mut dev, nr as i32) < 0 {
                return Err(FreenectError::new("Unable to open device"));
            }
            Ok(FreenectDevice::new(self, dev, self.use_video))
        }
    }

    /// Spawns a thread which process libfreenect's events. Only one of this thread can be spawned.
    pub fn spawn_process_thread(&self) -> Result<()> {
        let mut drop_sender = self.drop_sender.lock().unwrap();
        if let Some(ref sender) = *drop_sender {
            if let Err(_) = sender.send(()) {
                return Err(FreenectError::new("Cannot spawn process thread, thread is already \
                                               running"));
            }
        }
        struct Helper {
            ctx: *mut ffi::freenect_context,
        }
        unsafe impl Send for Helper {}
        let (s, r) = channel();
        *drop_sender = Some(s);
        let ctx = Helper { ctx: self.ctx };
        *self.thread_joiner.borrow_mut() = Some(thread::spawn(move || {
            'l: loop {
                match r.try_recv() {
                    Ok(_) => (),
                    Err(TryRecvError::Empty) => (),
                    Err(TryRecvError::Disconnected) => break 'l,
                }
                unsafe {
                    if ffi::freenect_process_events(ctx.ctx) < 0 {
                        // TODO: Throw error?
                        // The C++-Wrapper does the following thing:
                        // if (res < 0)
                        // {
                        // 	// libusb signals an error has occurred
                        // 	if (res == LIBUSB_ERROR_INTERRUPTED)
                        // 	{
                        // 		// This happens sometimes, it means that a system call in libusb was interrupted somehow (perhaps due to a signal)
                        // 		// The simple solution seems to be just ignore it.
                        // 		continue;
                        // 	}
                        break 'l;
                    }
                }
            }
        }));
        Ok(())
    }

    /// Stops the thread which process libfreenect's events
    pub fn stop_process_thread(&self) -> thread::Result<()> {
        let mut drop_sender = self.drop_sender.lock().unwrap();
        if let Some(sender) = drop_sender.take() {
            drop(sender);
        }
        if let Some(joiner) = self.thread_joiner.borrow_mut().take() {
            joiner.join()
        } else {
            // No Thread started
            Ok(())
        }
    }
}

impl Drop for FreenectContext {
    fn drop(&mut self) {
        self.stop_process_thread().unwrap();
        unsafe {
            ffi::freenect_shutdown(self.ctx);
        }
    }
}

/// Enumeration of available resolutions. See [here](https://zarvox.org/kinect/docs/libfreenect_8h.html#ac610d7d6fe91ecb4c54e3ff2d2525a58) for more information
pub enum FreenectResolution {
    Low,
    Medium,
    High,
}

impl FreenectResolution {
    fn to_c(self) -> ffi::freenect_resolution {
        match self {
            FreenectResolution::Low => ffi::freenect_resolution::FREENECT_RESOLUTION_LOW,
            FreenectResolution::Medium => ffi::freenect_resolution::FREENECT_RESOLUTION_MEDIUM,
            FreenectResolution::High => ffi::freenect_resolution::FREENECT_RESOLUTION_HIGH,
        }
    }
}

/// Enumeration of video formats. See [here](https://zarvox.org/kinect/docs/libfreenect_8h.html#ad651c9006cf1033b2246b49cae0b453a) for more information
pub enum FreenectVideoFormat {
    Rgb,
    Bayer,
    IR8,
    IR10,
    IR10Packed,
    YuvRgb,
    YuvRaw,
}

impl FreenectVideoFormat {
    fn to_c(self) -> ffi::freenect_video_format {
        match self {
            FreenectVideoFormat::Rgb => ffi::freenect_video_format::FREENECT_VIDEO_RGB,
            FreenectVideoFormat::Bayer => ffi::freenect_video_format::FREENECT_VIDEO_BAYER,
            FreenectVideoFormat::IR8 => ffi::freenect_video_format::FREENECT_VIDEO_IR_8BIT,
            FreenectVideoFormat::IR10 => ffi::freenect_video_format::FREENECT_VIDEO_IR_10BIT,
            FreenectVideoFormat::IR10Packed => {
                ffi::freenect_video_format::FREENECT_VIDEO_IR_10BIT_PACKED
            }
            FreenectVideoFormat::YuvRgb => ffi::freenect_video_format::FREENECT_VIDEO_YUV_RGB,
            FreenectVideoFormat::YuvRaw => ffi::freenect_video_format::FREENECT_VIDEO_YUV_RAW,
        }
    }
}

/// Enumeration of depth formats. See [here](https://zarvox.org/kinect/docs/libfreenect_8h.html#a258154182b56136a1c75a64ad5db6022) for more information
pub enum FreenectDepthFormat {
    Bit11,
    Bit10,
    Bit11Packed,
    Bit10Packed,
    Registered,
    MM,
}

impl FreenectDepthFormat {
    fn to_c(self) -> ffi::freenect_depth_format {
        match self {
            FreenectDepthFormat::Bit11 => ffi::freenect_depth_format::FREENECT_DEPTH_11BIT,
            FreenectDepthFormat::Bit10 => ffi::freenect_depth_format::FREENECT_DEPTH_10BIT,
            FreenectDepthFormat::Bit11Packed => {
                ffi::freenect_depth_format::FREENECT_DEPTH_11BIT_PACKED
            }
            FreenectDepthFormat::Bit10Packed => {
                ffi::freenect_depth_format::FREENECT_DEPTH_10BIT_PACKED
            }
            FreenectDepthFormat::Registered => {
                ffi::freenect_depth_format::FREENECT_DEPTH_REGISTERED
            }
            FreenectDepthFormat::MM => ffi::freenect_depth_format::FREENECT_DEPTH_MM,
        }
    }
}

/// Interacts with a freenect device (Kinect)
pub struct FreenectDevice<'a, 'b> {
    pub ctx: &'a FreenectContext,
    device: *mut ffi::freenect_device,
    use_video: bool,
    depth_sender: Mutex<Option<SyncSender<(&'b [u16], u32)>>>,
    video_sender: Mutex<Option<SyncSender<(&'b [u8], u32)>>>,
}


impl<'a, 'b> FreenectDevice<'a, 'b> {
    fn new(ctx: &'a FreenectContext,
           device: *mut ffi::freenect_device,
           use_video: bool)
           -> FreenectDevice {
        let res = FreenectDevice {
            ctx: ctx,
            device: device,
            use_video: use_video,
            depth_sender: Mutex::new(None),
            video_sender: Mutex::new(None),
        };
        unsafe {
            ffi::freenect_set_depth_callback(device, Some(depth_callback));
            if use_video {
                ffi::freenect_set_video_callback(device, Some(video_callback));
            }
        }
        res
    }

    /// Returns a stream-object for fetching depth data
    pub fn depth_stream(&'a self) -> Result<FreenectDepthStream<'a, 'b>> {
        unsafe {
            ffi::freenect_set_user(self.device, mem::transmute(self));
        }
        let mut d_sender = self.depth_sender.lock().unwrap();
        if d_sender.is_some() {
            return Err(FreenectError::new("Depth Stream already created"));
        }
        let (res, sender) = FreenectDepthStream::new(self)?;
        *d_sender = Some(sender);
        Ok(res)
    }

    pub fn set_depth_mode(&self,
                          resol: FreenectResolution,
                          format: FreenectDepthFormat)
                          -> Result<()> {
        unsafe {
            if ffi::freenect_set_depth_mode(self.device,
                                            ffi::freenect_find_depth_mode(resol.to_c(),
                                                                          format.to_c())) <
               0 {
                return Err(FreenectError::new("Unable to set depth mode"));
            }
        }
        Ok(())
    }

    pub fn set_video_mode(&self,
                          resol: FreenectResolution,
                          format: FreenectVideoFormat)
                          -> Result<()> {
        unsafe {
            if ffi::freenect_set_video_mode(self.device,
                                            ffi::freenect_find_video_mode(resol.to_c(),
                                                                          format.to_c())) <
               0 {
                return Err(FreenectError::new("Unable to change video mode"));
            }
        }
        Ok(())
    }

    /// Returns a stream-object for fetching rgb data
    pub fn video_stream(&'a self) -> Result<FreenectVideoStream<'a, 'b>> {
        unsafe {
            ffi::freenect_set_user(self.device, mem::transmute(self));
        }
        if !self.use_video {
            return Err(FreenectError::new("Cannot build video stream, context created without \
                                           support for it"));
        }
        let mut v_sender = self.video_sender.lock().unwrap();
        if v_sender.is_some() {
            return Err(FreenectError::new("Video Stream already created"));
        }
        let (res, sender) = FreenectVideoStream::new(self)?;
        *v_sender = Some(sender);
        Ok(res)
    }

    pub fn get_tilt_degree(&self) -> Result<f64> {
        unsafe {
            if ffi::freenect_update_tilt_state(self.device) < 0 {
                Err(FreenectError::new("Unable to update tilt state"))
            } else {
                let state = ffi::freenect_get_tilt_state(self.device);
                let degree = ffi::freenect_get_tilt_degs(state);
                Ok(degree)
            }
        }
    }

    pub fn set_tilt_degree(&self, degree: f64) -> Result<()> {
        unsafe {
            if ffi::freenect_set_tilt_degs(self.device, degree) < 0 {
                Err(FreenectError::new("Unable to set tilt degree"))
            } else {
                Ok(())
            }
        }
    }
}

impl<'a, 'b> Drop for FreenectDevice<'a, 'b> {
    fn drop(&mut self) {
        unsafe {
            ffi::freenect_close_device(self.device);
        }
    }
}

/// FreenectDepthStream should be used for fetching depth data from Kinect.
/// # Examples
/// ```rust,ignore
/// let dstream = device.depth_stream().unwrap();
/// if let Ok((data, timestamp)) = dstream.receiver.recv() {
///  // Fetch depth value for position x,y
///  let idx = y * 640 + x;
///  let depth_value = data[idx as usize];
/// //...
/// }
/// ```
pub struct FreenectDepthStream<'a, 'b>
    where 'b: 'a
{
    parent: &'a FreenectDevice<'a, 'b>,
    pub receiver: Receiver<(&'b [u16], u32)>,
}

impl<'a, 'b> FreenectDepthStream<'a, 'b> {
    fn new(parent: &'a FreenectDevice<'a, 'b>)
           -> Result<(FreenectDepthStream<'a, 'b>, SyncSender<(&'b [u16], u32)>)> {
        unsafe {
            if ffi::freenect_start_depth(parent.device) < 0 {
                return Err(FreenectError::new("Unable to start depth"));
            }
        }
        let (s, r) = sync_channel(2);
        Ok((FreenectDepthStream {
                parent: parent,
                receiver: r,
            },
            s))
    }
}

extern "C" fn depth_callback(dev: *mut ffi::freenect_device,
                             data: *mut std::os::raw::c_void,
                             timestamp: u32) {
    unsafe {
        let data = data as *mut u16;
        let data = slice::from_raw_parts(data, 640 * 480);
        let device = ffi::freenect_get_user(dev) as *mut FreenectDevice;
        let device = &*device;
        let sender = device.depth_sender.lock().unwrap();
        let sender = sender.as_ref().unwrap();
        match sender.try_send((data, timestamp)) {
            Err(TrySendError::Disconnected(_)) => panic!("Depth Channel is disconnected"),
            _ => (),
        }
    }
}

extern "C" fn video_callback(dev: *mut ffi::freenect_device,
                             data: *mut std::os::raw::c_void,
                             timestamp: u32) {
    unsafe {
        let data = data as *mut u8;
        let data = slice::from_raw_parts(data, 640 * 480 * 3);
        let device = ffi::freenect_get_user(dev) as *mut FreenectDevice;
        let device = &*device;
        let sender = device.video_sender.lock().unwrap();
        let sender = sender.as_ref().unwrap();
        match sender.try_send((data, timestamp)) {
            Err(TrySendError::Disconnected(_)) => panic!("Video Channel is disconnected"),
            _ => (),
        }
    }
}

impl<'a, 'b> Drop for FreenectDepthStream<'a, 'b> {
    fn drop(&mut self) {
        unsafe {
            ffi::freenect_stop_depth(self.parent.device);
        }
        *self.parent.depth_sender.lock().unwrap() = None;
    }
}

/// FreenectVideoStream should be used for fetching rgb data from Kinect.
/// # Examples
/// ```rust,ignore
/// let dstream = device.depth_stream().unwrap();
/// if let Ok((data, timestamp)) = dstream.receiver.recv() {
///  // Fetch rgb value for position x,y
///  let idx = 3 * (y * 640 + x) as usize;
///  let (r, g, b) = (data[idx], data[idx + 1], data[idx + 2]);
/// //...
/// }
/// ```
pub struct FreenectVideoStream<'a, 'b>
    where 'b: 'a
{
    parent: &'a FreenectDevice<'a, 'b>,
    pub receiver: Receiver<(&'b [u8], u32)>,
}
impl<'a, 'b> FreenectVideoStream<'a, 'b> {
    fn new(parent: &'a FreenectDevice<'a, 'b>)
           -> Result<(FreenectVideoStream<'a, 'b>, SyncSender<(&'b [u8], u32)>)> {
        unsafe {
            if ffi::freenect_start_video(parent.device) < 0 {
                return Err(FreenectError::new("Unable to start video"));
            }
        }
        let (s, r) = sync_channel(2);
        Ok((FreenectVideoStream {
                parent: parent,
                receiver: r,
            },
            s))
    }
}
impl<'a, 'b> Drop for FreenectVideoStream<'a, 'b> {
    fn drop(&mut self) {
        unsafe {
            ffi::freenect_stop_video(self.parent.device);
        }
        *self.parent.video_sender.lock().unwrap() = None;
    }
}
