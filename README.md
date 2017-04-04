# Freenect-rs

freenect-rs is a rust wrapper to interact with [libfreenect](https://github.com/OpenKinect/libfreenect).
It can be used to fetch rgb and depth data from Kinect and to control its motor.

## Example

The example directory contains a more complete example.

```rust
use freenectrs::freenect;
// Init with video functionality
let ctx = freenect::FreenectContext::init_with_video().unwrap();
// Open first device
let device = ctx.open_device(0).unwrap();
// Setup mode for this device
device.set_depth_mode(freenect::FreenectResolution::Medium, freenect::FreenectDepthFormat::MM).unwrap();
device.set_video_mode(freenect::FreenectResolution::Medium, freenect::FreenectVideoFormat::Rgb).unwrap();
// Get rgb and depth stream
let dstream = device.depth_stream().unwrap();
let vstream = device.video_stream().unwrap();
// Start the main-loop-thread
ctx.spawn_process_thread().unwrap();
// Fetch the video and depth frames
if let Ok((data, timestamp)) = dstream.receiver.try_recv() {
       // ... handle depth data
}
if let Ok((data, timestamp)) = vstream.receiver.try_recv() {
       // ... handle rgb data
}
ctx.stop_process_thread().unwrap();
```
