use slint::ComponentHandle;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::device::BodycamDevice;
use crate::camera::{CameraDevice, AudioDevice, CameraManager};

// slint::include_modules!(); // Disabled for compilation

pub struct BodycamUI {
    // ui: MainWindow, // Disabled for compilation
    config: Arc<Mutex<Config>>,
    device: Arc<Mutex<BodycamDevice>>,
    camera_manager: Arc<Mutex<CameraManager>>,
    config_path: PathBuf,
}

impl BodycamUI {
    pub fn new(
        config: Config,
        device: BodycamDevice,
        config_path: Option<PathBuf>,
    ) -> Result<Self> {
        let config_path = config_path.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("config.toml")
        });
        
        let camera_manager = CameraManager::new()?;
        
        let ui = MainWindow::new()?;
        let config = Arc::new(Mutex::new(config));
        let device = Arc::new(Mutex::new(device));
        let camera_manager = Arc::new(Mutex::new(camera_manager));
        
        let mut ui_instance = Self {
            ui,
            config: Arc::clone(&config),
            device: Arc::clone(&device),
            camera_manager: Arc::clone(&camera_manager),
            config_path,
        };
        
        ui_instance.setup_ui_callbacks()?;
        ui_instance.load_initial_settings()?;
        
        Ok(ui_instance)
    }

    fn setup_ui_callbacks(&mut self
    ) -> Result<()> {
        let device = Arc::clone(&self.device);
        let config = Arc::clone(&self.config);
        let config_path = self.config_path.clone();
        
        // Record button
        self.ui.on_record_button_pressed({
            let device = Arc::clone(&device);
            move || {
                let device = device.clone();
                tokio::spawn(async move {
                    let device = device.lock().unwrap();
                    if device.is_recording {
                        let _ = device.stop_recording().await;
                    } else {
                        let _ = device.start_recording(None, None).await;
                    }
                });
            }
        });
        
        // Emergency button
        self.ui.on_emergency_button_pressed({
            let device = Arc::clone(&device);
            move || {
                let device = device.clone();
                tokio::spawn(async move {
                    let device = device.lock().unwrap();
                    let _ = device.trigger_incident("emergency", "high").await;
                });
            }
        });
        
        // Settings callbacks
        self.ui.on_camera_changed({
            let config = Arc::clone(&config);
            let config_path = config_path.clone();
            move |camera| {
                let config = config.clone();
                let config_path = config_path.clone();
                tokio::spawn(async move {
                    let mut config = config.lock().unwrap();
                    config.hardware.camera_index = Some(camera.parse().unwrap_or(0));
                    let _ = config.save(&config_path).await;
                });
            }
        });
        
        self.ui.on_audio_changed({
            let config = Arc::clone(&config);
            let config_path = config_path.clone();
            move |audio| {
                let config = config.clone();
                let config_path = config_path.clone();
                tokio::spawn(async move {
                    let mut config = config.lock().unwrap();
                    config.audio.device_path = audio;
                    let _ = config.save(&config_path).await;
                });
            }
        });
        
        self.ui.on_resolution_changed({
            let config = Arc::clone(&config);
            let config_path = config_path.clone();
            move |resolution| {
                let config = config.clone();
                let config_path = config_path.clone();
                tokio::spawn(async move {
                    let mut config = config.lock().unwrap();
                    config.recording.resolution = resolution;
                    let _ = config.save(&config_path).await;
                });
            }
        });
        
        self.ui.on_fps_changed({
            let config = Arc::clone(&config);
            let config_path = config_path.clone();
            move |fps| {
                let config = config.clone();
                let config_path = config_path.clone();
                tokio::spawn(async move {
                    let mut config = config.lock().unwrap();
                    config.recording.fps = fps.parse().unwrap_or(30);
                    let _ = config.save(&config_path).await;
                });
            }
        });
        
        Ok(())
    }

    fn load_initial_settings(&mut self
    ) -> Result<()> {
        let config = self.config.lock().unwrap();
        let camera_manager = self.camera_manager.lock().unwrap();
        
        let cameras = camera_manager.get_cameras();
        let audio_devices = camera_manager.get_audio_devices();
        
        let camera_names: Vec<String> = cameras.iter()
            .map(|c| c.name.clone())
            .collect();
            
        let audio_names: Vec<String> = audio_devices.iter()
            .map(|a| a.name.clone())
            .collect();
        
        self.ui.set_cameras(slint::ModelRc::from(slint::VecModel::from(camera_names)));
        self.ui.set_audio_devices(slint::ModelRc::from(slint::VecModel::from(audio_names)));
        
        self.ui.set_is_simulation(config.simulation.enabled);
        
        Ok(())
    }

    pub async fn run(&self
    ) -> Result<()> {
        self.ui.run()?;
        Ok(())
    }

    pub fn update_status(&self, status: &str
    ) {
        self.ui.set_status_text(status.into());
    }

    pub fn update_battery(&self, level: f32
    ) {
        self.ui.set_battery_level(format!("{:.0}%", level));
    }

    pub fn update_storage(&self, total: u64, used: u64
    ) {
        let available = total - used;
        let available_gb = available as f64 / 1_000_000_000.0;
        self.ui.set_storage_info(format!("{:.1}GB available", available_gb));
    }

    pub fn update_recording_status(&self, is_recording: bool
    ) {
        self.ui.set_is_recording(is_recording);
    }

    pub fn update_time(&self, time: &str
    ) {
        self.ui.set_current_time(time.into());
    }

    pub fn show_emergency_alert(&self
    ) {
        self.ui.set_emergency_active(true);
    }

    pub fn hide_emergency_alert(&self
    ) {
        self.ui.set_emergency_active(false);
    }
}