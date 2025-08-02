use anyhow::{Result, Context};
use nokhwa::{Camera, CameraFormat, CaptureAPIBackend, FrameFormat};
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gstreamer::prelude::*;
use gstreamer::{Element, ElementFactory, Pipeline, State};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct CameraDevice {
    pub name: String,
    pub index: u32,
    pub formats: Vec<CameraFormat>,
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub index: u32,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct CameraManager {
    cameras: Vec<CameraDevice>,
    audio_devices: Vec<AudioDevice>,
    current_camera: Option<Camera>,
    current_audio: Option<cpai::Device>,
    pipeline: Option<Pipeline>,
    is_recording: bool,
}

impl CameraManager {
    pub fn new() -> Result<Self> {
        let cameras = Self::enumerate_cameras()?;
        let audio_devices = Self::enumerate_audio_devices()?;
        
        gstreamer::init()?;
        
        Ok(Self {
            cameras,
            audio_devices,
            current_camera: None,
            current_audio: None,
            pipeline: None,
            is_recording: false,
        })
    }

    fn enumerate_cameras() -> Result<Vec<CameraDevice>> {
        let mut cameras = Vec::new();
        
        // Enumerate cameras using nokhwa
        let cameras_info = nokhwa::query(CaptureAPIBackend::Auto)?;
        
        for (i, camera_info) in cameras_info.iter().enumerate() {
            let formats = camera_info.formats().to_vec();
            
            cameras.push(CameraDevice {
                name: camera_info.human_name().unwrap_or_else(|| format!("Camera {}", i)),
                index: i as u32,
                formats,
            });
        }
        
        // Common Linux camera devices
        #[cfg(target_os = "linux")]
        {
            for i in 0..10 {
                let path = format!("/dev/video{}", i);
                if std::path::Path::new(&path).exists() {
                    let mut found = false;
                    for camera in &cameras {
                        if camera.index == i {
                            found = true;
                            break;
                        }
                    }
                    
                    if !found {
                        cameras.push(CameraDevice {
                            name: format!("Linux Camera {}/dev/video{}", i, i),
                            index: i as u32,
                            formats: vec![
                                CameraFormat::new(1920, 1080, FrameFormat::MJPEG, 30),
                                CameraFormat::new(1280, 720, FrameFormat::MJPEG, 30),
                                CameraFormat::new(640, 480, FrameFormat::MJPEG, 30),
                            ],
                        });
                    }
                }
            }
        }
        
        // Common macOS camera devices
        #[cfg(target_os = "macos")]
        {
            // FaceTime HD Camera
            cameras.push(CameraDevice {
                name: "FaceTime HD Camera".to_string(),
                index: 0,
                formats: vec![
                    CameraFormat::new(1280, 720, FrameFormat::NV12, 30),
                    CameraFormat::new(640, 480, FrameFormat::NV12, 30),
                ],
            });
            
            // USB cameras
            for i in 1..5 {
                cameras.push(CameraDevice {
                    name: format!("USB Camera {}", i),
                    index: i as u32,
                    formats: vec![
                        CameraFormat::new(1920, 1080, FrameFormat::MJPEG, 30),
                        CameraFormat::new(1280, 720, FrameFormat::MJPEG, 30),
                    ],
                });
            }
        }
        
        Ok(cameras)
    }

    fn enumerate_audio_devices() -> Result<Vec<AudioDevice>> {
        let host = cpal::default_host();
        let mut audio_devices = Vec::new();
        
        if let Ok(input_devices) = host.input_devices() {
            for (i, device) in input_devices.enumerate() {
                if let Ok(name) = device.name() {
                    let default_config = device.default_input_config().ok();
                    
                    audio_devices.push(AudioDevice {
                        name,
                        index: i as u32,
                        sample_rate: default_config.as_ref().map_or(44100, |c| c.sample_rate().0),
                        channels: default_config.as_ref().map_or(2, |c| c.channels() as u16),
                    });
                }
            }
        }
        
        // Common Linux audio devices
        #[cfg(target_os = "linux")]
        {
            audio_devices.push(AudioDevice {
                name: "Built-in Microphone".to_string(),
                index: 1000,
                sample_rate: 44100,
                channels: 2,
            });
            
            audio_devices.push(AudioDevice {
                name: "USB Audio Device".to_string(),
                index: 1001,
                sample_rate: 44100,
                channels: 1,
            });
        }
        
        // Common macOS audio devices
        #[cfg(target_os = "macos")]
        {
            audio_devices.push(AudioDevice {
                name: "Built-in Microphone".to_string(),
                index: 2000,
                sample_rate: 44100,
                channels: 1,
            });
            
            audio_devices.push(AudioDevice {
                name: "AirPods Microphone".to_string(),
                index: 2001,
                sample_rate: 16000,
                channels: 1,
            });
        }
        
        Ok(audio_devices)
    }

    pub fn get_cameras(&self) -> &Vec<CameraDevice> {
        &self.cameras
    }

    pub fn get_audio_devices(&self) -> &Vec<AudioDevice> {
        &self.audio_devices
    }

    pub fn start_camera(&mut self, camera_index: u32, format: CameraFormat) -> Result<()> {
        let camera = Camera::new(
            CameraIndex::Index(camera_index),
            RequestedFormat::new(RequestedFormatType::Closest(format)),
        )?;
        
        camera.open_stream()?;
        self.current_camera = Some(camera);
        
        Ok(())
    }

    pub fn stop_camera(&mut self
    ) -> Result<()> {
        if let Some(camera) = &mut self.current_camera {
            camera.stop_stream()?;
        }
        self.current_camera = None;
        Ok(())
    }

    pub fn start_recording(
        &mut self,
        camera_index: u32,
        audio_index: u32,
        resolution: (u32, u32),
        fps: u32,
        output_file: &str,
    ) -> Result<()> {
        let pipeline = Pipeline::new(None)?;
        
        // Video source
        let video_source = ElementFactory::make("v4l2src")
            .property("device", format!("/dev/video{}", camera_index))
            .build()?;
            
        // Video caps
        let video_caps = ElementFactory::make("capsfilter")
            .property("caps", &gstreamer::Caps::builder("video/x-raw")
                .field("width", resolution.0 as i32)
                .field("height", resolution.1 as i32)
                .field("framerate", gstreamer::Fraction::new(fps as i32, 1))
                .build())
            .build()?;
            
        // Video encoder
        let video_encoder = ElementFactory::make("x264enc")
            .property("bitrate", 5000)
            .build()?;
            
        // Audio source
        let audio_source = ElementFactory::make("alsasrc")
            .build()?;
            
        // Audio encoder
        let audio_encoder = ElementFactory::make("faac")
            .build()?;
            
        // Muxer
        let muxer = ElementFactory::make("mp4mux")
            .build()?;
            
        // Filesink
        let filesink = ElementFactory::make("filesink")
            .property("location", output_file)
            .build()?;
            
        // Add elements to pipeline
        pipeline.add_many([&video_source, &video_caps, &video_encoder,
                          &audio_source, &audio_encoder,
                          &muxer, &filesink])?;
        
        // Link elements
        video_source.link(&video_caps)?;
        video_caps.link(&video_encoder)?;
        video_encoder.link_pads(None, &muxer, Some("video_0"))?;
        
        audio_source.link(&audio_encoder)?;
        audio_encoder.link_pads(None, &muxer, Some("audio_0"))?;
        
        muxer.link(&filesink)?;
        
        pipeline.set_state(State::Playing)?;
        self.pipeline = Some(pipeline);
        self.is_recording = true;
        
        Ok(())
    }

    pub fn start_simulated_recording(
        &mut self,
        output_file: &str,
    ) -> Result<()> {
        // Create test video using videotestsrc
        let pipeline = Pipeline::new(None)?;
        
        let video_source = ElementFactory::make("videotestsrc")
            .property("pattern", 0) // Smpte pattern
            .build()?;
            
        let video_caps = ElementFactory::make("capsfilter")
            .property("caps", &gstreamer::Caps::builder("video/x-raw")
                .field("width", 1920i32)
                .field("height", 1080i32)
                .field("framerate", gstreamer::Fraction::new(30, 1))
                .build())
            .build()?;
            
        let video_encoder = ElementFactory::make("x264enc")
            .property("bitrate", 2000)
            .build()?;
            
        let audio_source = ElementFactory::make("audiotestsrc")
            .property("wave", 0) // Sine wave
            .build()?;
            
        let audio_encoder = ElementFactory::make("faac")
            .build()?;
            
        let muxer = ElementFactory::make("mp4mux")
            .build()?;
            
        let filesink = ElementFactory::make("filesink")
            .property("location", output_file)
            .build()?;
            
        pipeline.add_many([&video_source, &video_caps, &video_encoder,
                          &audio_source, &audio_encoder,
                          &muxer, &filesink])?;
        
        video_source.link(&video_caps)?;
        video_caps.link(&video_encoder)?;
        video_encoder.link_pads(None, &muxer, Some("video_0"))?;
        
        audio_source.link(&audio_encoder)?;
        audio_encoder.link_pads(None, &muxer, Some("audio_0"))?;
        
        muxer.link(&filesink)?;
        
        pipeline.set_state(State::Playing)?;
        self.pipeline = Some(pipeline);
        self.is_recording = true;
        
        Ok(())
    }

    pub fn stop_recording(&mut self
    ) -> Result<()> {
        if let Some(pipeline) = &self.pipeline {
            pipeline.send_event(gstreamer::event::Eos::new());
            pipeline.set_state(State::Null)?;
        }
        self.pipeline = None;
        self.is_recording = false;
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }
}