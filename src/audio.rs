use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::path::PathBuf;
use uuid::Uuid;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPlaybackRequest {
    pub source: AudioSource,
    pub volume: Option<f32>,
    pub loop_playback: Option<bool>,
    pub priority: AudioPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioSource {
    CustomFile {
        file_path: String,
    },
    PresetFile {
        file_id: String,
    },
    TtsLocal {
        text: String,
        voice: Option<String>,
        rate: Option<u32>,
    },
    TtsRemote {
        text: String,
        provider: TtsProvider,
        voice: Option<String>,
        api_key: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TtsProvider {
    Google,
    Amazon,
    Microsoft,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStatus {
    pub is_playing: bool,
    pub current_source: Option<String>,
    pub volume: f32,
    pub playback_id: Option<String>,
}

pub struct AudioManager {
    config: Config,
    preset_files: std::collections::HashMap<String, PathBuf>,
}

impl AudioManager {
    pub fn new(config: Config) -> Self {
        let mut preset_files = std::collections::HashMap::new();
        
        // Add default preset files
        preset_files.insert("beep".to_string(), PathBuf::from("/usr/share/sounds/beep.wav"));
        preset_files.insert("alert".to_string(), PathBuf::from("/usr/share/sounds/alert.wav"));
        preset_files.insert("warning".to_string(), PathBuf::from("/usr/share/sounds/warning.wav"));
        preset_files.insert("emergency".to_string(), PathBuf::from("/usr/share/sounds/emergency.wav"));
        preset_files.insert("start".to_string(), PathBuf::from("/usr/share/sounds/start.wav"));
        preset_files.insert("stop".to_string(), PathBuf::from("/usr/share/sounds/stop.wav"));
        
        Self {
            config,
            preset_files,
        }
    }

    pub async fn play_audio(&self, request: AudioPlaybackRequest) -> Result<String> {
        let playback_id = Uuid::new_v4().to_string();
        
        match request.source {
            AudioSource::CustomFile { file_path } => {
                self.play_custom_file(&file_path, request.volume, request.loop_playback).await?;
            }
            AudioSource::PresetFile { file_id } => {
                self.play_preset_file(&file_id, request.volume, request.loop_playback).await?;
            }
            AudioSource::TtsLocal { text, voice, rate } => {
                self.play_tts_local(&text, voice.as_deref(), rate, request.volume).await?;
            }
            AudioSource::TtsRemote { text, provider, voice, api_key } => {
                self.play_tts_remote(&text, provider, voice.as_deref(), api_key.as_deref(), request.volume).await?;
            }
        }
        
        Ok(playback_id)
    }

    pub async fn stop_audio(&self) -> Result<()> {
        // Use pkill to stop any running audio players
        let _ = Command::new("pkill")
            .arg("aplay")
            .status()
            .await;
            
        let _ = Command::new("pkill")
            .arg("mpg123")
            .status()
            .await;
            
        let _ = Command::new("pkill")
            .arg("ffplay")
            .status()
            .await;
            
        Ok(())
    }

    pub async fn get_status(&self) -> Result<AudioStatus> {
        // Check if any audio is currently playing
        let is_playing = self.is_audio_playing().await?;
        
        Ok(AudioStatus {
            is_playing,
            current_source: None, // In a real implementation, track the current source
            volume: 1.0, // Default volume
            playback_id: None,
        })
    }

    pub async fn set_volume(&self, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        
        // Use amixer to set system volume
        let volume_percent = (volume * 100.0) as u32;
        
        let status = Command::new("amixer")
            .arg("set")
            .arg("Master")
            .arg(format!("{}%", volume_percent))
            .status()
            .await?;
            
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to set volume"));
        }
        
        Ok(())
    }

    async fn play_custom_file(&self, file_path: &str, volume: Option<f32>, loop_playback: Option<bool>) -> Result<()> {
        let path = PathBuf::from(file_path);
        
        if !path.exists() {
            return Err(anyhow::anyhow!("Audio file not found: {}", file_path));
        }
        
        if let Some(vol) = volume {
            self.set_volume(vol).await?;
        }
        
        let mut cmd = Command::new("aplay");
        
        if loop_playback.unwrap_or(false) {
            cmd.arg("--repeat");
        }
        
        cmd.arg(path);
        
        let status = cmd.status().await?;
        
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to play audio file"));
        }
        
        Ok(())
    }

    async fn play_preset_file(&self, file_id: &str, volume: Option<f32>, loop_playback: Option<bool>) -> Result<()> {
        let file_path = self.preset_files.get(file_id)
            .ok_or_else(|| anyhow::anyhow!("Preset file not found: {}", file_id))?;
        
        self.play_custom_file(file_path.to_string_lossy().as_ref(), volume, loop_playback).await
    }

    async fn play_tts_local(&self, text: &str, voice: Option<&str>, rate: Option<u32>, volume: Option<f32>) -> Result<()> {
        // Use espeak for local TTS
        let mut cmd = Command::new("espeak");
        
        if let Some(voice) = voice {
            cmd.arg("-v").arg(voice);
        }
        
        if let Some(rate) = rate {
            cmd.arg("-s").arg(rate.to_string());
        }
        
        cmd.arg(text);
        
        let status = cmd.status().await?;
        
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to play TTS"));
        }
        
        Ok(())
    }

    async fn play_tts_remote(&self, text: &str, provider: TtsProvider, voice: Option<&str>, api_key: Option<&str>, volume: Option<f32>) -> Result<()> {
        // Generate TTS audio based on provider
        let audio_data = match provider {
            TtsProvider::Google => self.generate_google_tts(text, voice, api_key).await?,
            TtsProvider::Amazon => self.generate_amazon_tts(text, voice, api_key).await?,
            TtsProvider::Microsoft => self.generate_microsoft_tts(text, voice, api_key).await?,
            TtsProvider::OpenAI => self.generate_openai_tts(text, voice, api_key).await?,
        };
        
        // Save to temporary file and play
        let temp_path = std::env::temp_dir().join(format!("tts_{}.mp3", Uuid::new_v4()));
        tokio::fs::write(&temp_path, audio_data).await?;
        
        self.play_custom_file(temp_path.to_string_lossy().as_ref(), volume, None).await?;
        
        // Clean up
        tokio::fs::remove_file(temp_path).await?;
        
        Ok(())
    }

    async fn generate_google_tts(&self, text: &str, voice: Option<&str>, api_key: Option<&str>) -> Result<Vec<u8>> {
        let api_key = api_key.ok_or_else(|| anyhow::anyhow!("Google TTS requires API key"))?;
        let voice = voice.unwrap_or("en-US-Wavenet-D");
        
        // This is a placeholder - implement actual Google TTS API call
        Err(anyhow::anyhow!("Google TTS implementation pending"))
    }

    async fn generate_amazon_tts(&self, text: &str, voice: Option<&str>, api_key: Option<&str>) -> Result<Vec<u8>> {
        // Placeholder for Amazon Polly
        Err(anyhow::anyhow!("Amazon TTS implementation pending"))
    }

    async fn generate_microsoft_tts(&self, text: &str, voice: Option<&str>, api_key: Option<&str>) -> Result<Vec<u8>> {
        // Placeholder for Microsoft Azure TTS
        Err(anyhow::anyhow!("Microsoft TTS implementation pending"))
    }

    async fn generate_openai_tts(&self, text: &str, voice: Option<&str>, api_key: Option<&str>) -> Result<Vec<u8>> {
        // Placeholder for OpenAI TTS
        Err(anyhow::anyhow!("OpenAI TTS implementation pending"))
    }

    async fn is_audio_playing(&self) -> Result<bool> {
        // Check if any audio players are running
        let output = Command::new("pgrep")
            .arg("-f")
            .arg("aplay|mpg123|ffplay")
            .output()
            .await?;
            
        Ok(!output.stdout.is_empty())
    }
}