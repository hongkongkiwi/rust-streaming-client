import { Button, CheckBox, ComboBox, Slider, VerticalBox, HorizontalBox, GridBox, TextEdit, LineEdit } from "std-widgets.slint";

export component MainWindow inherits Window {
    min-width: 800px;
    min-height: 600px;
    title: "PatrolSight Bodycam";
    
    in-out property <string> status-text: "Ready";
    in-out property <string> battery-level: "100%";
    in-out property <string> storage-info: "64GB available";
    in-out property <string> current-time: "00:00:00";
    in-out property <bool> is-recording: false;
    in-out property <bool> is-streaming: false;
    in-out property <bool> is-simulation: false;
    in-out property <bool> emergency-active: false;
    
    in-out property <[slint::Model<string>]> cameras: [
        "Default Camera", "USB Camera", "Built-in Camera"
    ];
    in-out property <[slint::Model<string>]> audio_devices: [
        "Default Audio", "USB Microphone", "Built-in Mic"
    ];
    
    in-out property <string> selected-camera: "Default Camera";
    in-out property <string> selected-audio: "Default Audio";
    in-out property <string> selected-resolution: "1920x1080";
    in-out property <string> selected-fps: "30";
    
    callback record-button-pressed();
    callback stop-button-pressed();
    callback emergency-button-pressed();
    callback settings-changed();
    callback camera-changed(string);
    callback audio-changed(string);
    callback resolution-changed(string);
    callback fps-changed(string);
    
    VerticalBox {
        spacing: 10px;
        padding: 20px;
        
        // Header with status
        HorizontalBox {
            alignment: center;
            Text {
                text: "PatrolSight Bodycam";
                font-size: 24px;
                font-weight: bold;
                color: #2c3e50;
            }
        }
        
        // Main display area
        HorizontalBox {
            spacing: 20px;
            
            // Left panel - Camera preview
            VerticalBox {
                spacing: 10px;
                
                Rectangle {
                    width: 400px;
                    height: 300px;
                    border-radius: 10px;
                    border-width: 2px;
                    border-color: #3498db;
                    background: is-recording ? #e74c3c : #34495e;
                    
                    Text {
                        text: is-recording ? "🔴 RECORDING" : "📹 CAMERA OFF";
                        color: white;
                        font-size: 18px;
                        font-weight: bold;
                    }
                }
                
                // Recording controls
                HorizontalBox {
                    spacing: 10px;
                    alignment: center;
                    
                    Button {
                        text: is-recording ? "Stop Recording" : "Start Recording";
                        enabled: !is-streaming;
                        clicked => {
                            record-button-pressed();
                        }
                        
                        background: is-recording ? #e74c3c : #2ecc71;
                        color: white;
                    }
                    
                    Button {
                        text: emergency-active ? "Cancel Emergency" : "Emergency";
                        clicked => {
                            emergency-button-pressed();
                        }
                        background: emergency-active ? #f39c12 : #e74c3c;
                        color: white;
                    }
                }
            }
            
            // Right panel - Status and settings
            VerticalBox {
                spacing: 15px;
                
                // Status info
                GridBox {
                    spacing: 5px;
                    columns: 2;
                    
                    Text { text: "Status:"; font-weight: bold; }
                    Text { text: root.status-text; color: #2c3e50; }
                    
                    Text { text: "Battery:"; font-weight: bold; }
                    Text { text: root.battery-level; color: #27ae60; }
                    
                    Text { text: "Storage:"; font-weight: bold; }
                    Text { text: root.storage-info; color: #3498db; }
                    
                    Text { text: "Time:"; font-weight: bold; }
                    Text { text: root.current-time; color: #2c3e50; }
                }
                
                // Camera settings
                GroupBox {
                    title: "Camera Settings";
                    
                    VerticalBox {
                        spacing: 5px;
                        
                        Text { text: "Camera Device:"; }
                        ComboBox {
                            model: root.cameras;
                            current-value: selected-camera;
                            current-value-changed => (value) => {
                                camera-changed(value);
                            }
                        }
                        
                        Text { text: "Audio Device:"; }
                        ComboBox {
                            model: root.audio_devices;
                            current-value: selected-audio;
                            current-value-changed => (value) => {
                                audio-changed(value);
                            }
                        }
                        
                        Text { text: "Resolution:"; }
                        ComboBox {
                            model: [
                                "1920x1080", "1280x720", "640x480", "3840x2160"
                            ];
                            current-value: selected-resolution;
                            current-value-changed => (value) => {
                                resolution-changed(value);
                            }
                        }
                        
                        Text { text: "FPS:"; }
                        ComboBox {
                            model: [
                                "30", "60", "25", "24", "15"
                            ];
                            current-value: selected-fps;
                            current-value-changed => (value) => {
                                fps-changed(value);
                            }
                        }
                    }
                }
                
                // Recording settings
                GroupBox {
                    title: "Recording Settings";
                    
                    VerticalBox {
                        spacing: 5px;
                        
                        CheckBox {
                            text: "Simulation Mode";
                            checked: is-simulation;
                        }
                        
                        CheckBox {
                            text: "Enable Audio";
                            checked: true;
                        }
                        
                        CheckBox {
                            text: "Auto Upload";
                            checked: true;
                        }
                        
                        CheckBox {
                            text: "Encryption";
                            checked: true;
                        }
                    }
                }
                
                // Network status
                GroupBox {
                    title: "Network Status";
                    
                    VerticalBox {
                        spacing: 5px;
                        
                        Text { text: "Connected: ✅"; color: #27ae60; }
                        Text { text: "Upload Speed: 5 Mbps"; color: #3498db; }
                        Text { text: "Signal: Strong"; color: #27ae60; }
                    }
                }
            }
        }
        
        // Footer
        HorizontalBox {
            alignment: center;
            Text {
                text: "PatrolSight Security Systems";
                font-size: 12px;
                color: #7f8c8d;
            }
        }
    }
}