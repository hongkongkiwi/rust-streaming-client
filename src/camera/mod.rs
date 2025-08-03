use anyhow::{Result, Context};
use nokhwa::Camera;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType, CameraFormat, FrameFormat};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct CameraDevice {
    pub index: u32,
    pub name: String,
    pub capabilities: Vec<CameraFormat>,
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub index: u32,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct CameraManager {
    cameras: Vec<CameraDevice>,
    audio_devices: Vec<AudioDevice>,
    current_camera: Option<Camera>,
    current_audio: Option<cpal::Device>,
    is_recording: bool,
}

impl CameraManager {
    pub fn new() -> Result<Self> {
        let cameras = Self::enumerate_cameras()?;
        let audio_devices = Self::enumerate_audio_devices()?;
        
        Ok(Self {
            cameras,
            audio_devices,
            current_camera: None,
            current_audio: None,
            is_recording: false,
        })
    }

    fn enumerate_cameras() -> Result<Vec<CameraDevice>> {
        let mut cameras = Vec::new();
        
        // Enumerate available cameras using nokhwa
        let available_cameras = nokhwa::query(nokhwa::utils::ApiBackend::Auto)
            .context("Failed to query cameras")?;
            
        for (index, camera_info) in available_cameras.iter().enumerate() {
            // Get camera capabilities
            let mut capabilities = Vec::new();
            
            // Try to get camera formats
            if let Ok(camera) = Camera::new(
                camera_info.index().clone(),
                RequestedFormat::new::<FrameFormat>(RequestedFormatType::AbsoluteHighestFrameRate)
            ) {
                // Add basic capability
                capabilities.push(CameraFormat::new(
                    nokhwa::utils::Resolution::new(640, 480),
                    FrameFormat::MJPEG,
                    30
                ));
                capabilities.push(CameraFormat::new(
                    nokhwa::utils::Resolution::new(1920, 1080),
                    FrameFormat::MJPEG,
                    30
                ));
            }
            
            cameras.push(CameraDevice {
                index: index as u32,
                name: camera_info.human_name().to_string(),
                capabilities,
            });
        }
        
        Ok(cameras)
    }

    fn enumerate_audio_devices() -> Result<Vec<AudioDevice>> {
        let mut audio_devices = Vec::new();
        let host = cpal::default_host();
        
        // Get input devices
        let devices = host.input_devices()
            .context("Failed to get input devices")?;
            
        for (index, device) in devices.enumerate() {
            if let Ok(name) = device.name() {
                let default_config = device.default_input_config()
                    .unwrap_or_else(|_| cpal::SupportedStreamConfig::new(
                        2,
                        cpal::SampleRate(44100),
                        cpal::SupportedBufferSize::Range { min: 512, max: 8192 },
                        cpal::SampleFormat::F32
                    ));
                    
                audio_devices.push(AudioDevice {
                    index: index as u32,
                    name,
                    sample_rate: default_config.sample_rate().0,
                    channels: default_config.channels(),
                });
            }
        }
        
        Ok(audio_devices)
    }

    pub fn get_cameras(&self) -> &[CameraDevice] {
        &self.cameras
    }

    pub fn get_audio_devices(&self) -> &[AudioDevice] {
        &self.audio_devices
    }

    pub fn start_camera(&mut self, camera_index: u32) -> Result<()> {
        let camera_info = self.cameras.get(camera_index as usize)
            .ok_or_else(|| anyhow::anyhow!("Camera index {} not found", camera_index))?;
            
        let camera = Camera::new(
            CameraIndex::Index(camera_index),
            RequestedFormat::new::<FrameFormat>(RequestedFormatType::AbsoluteHighestFrameRate)
        )?;
        
        self.current_camera = Some(camera);
        tracing::info!("Camera {} started: {}", camera_index, camera_info.name);
        Ok(())
    }

    pub fn stop_camera(&mut self) -> Result<()> {
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
        // Note: Primary recording functionality is implemented in media.rs using FFmpeg
        // This function is kept for compatibility but delegates to the main implementation
        tracing::info!("Camera recording requested - use MediaRecorder in media.rs for full functionality");
        Err(anyhow::anyhow!("Use MediaRecorder for recording - this is a camera management interface only"))
    }

    pub fn start_simulated_recording(
        &mut self,
        output_file: &str,
    ) -> Result<()> {
        // Simulated recording would use FFmpeg videotestsrc instead of GStreamer
        tracing::info!("Simulated recording requested for: {}", output_file);
        Err(anyhow::anyhow!("Simulated recording not implemented without GStreamer - use FFmpeg alternative"))
    }

    pub fn stop_recording(&mut self) -> Result<()> {
        // Remove pipeline dependency - recording managed in MediaRecorder
        self.is_recording = false;
        tracing::info!("Camera recording stopped - actual recording managed by MediaRecorder");
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }
}