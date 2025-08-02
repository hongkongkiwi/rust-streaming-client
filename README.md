# Streaming Client - Rust Implementation

A Rust-based body camera client for the PatrolSight security monitoring platform. This client simulates a body camera device with real-time recording, incident detection, and hardware integration capabilities.

## Features

- **Device Provisioning**: Register devices with the PatrolSight platform
- **Real-time Recording**: Capture video and audio with configurable settings
- **Hardware Integration**: GPIO, LED, button, and sensor support for Linux systems
- **Simulation Mode**: Test functionality on macOS or without hardware
- **Incident Detection**: Automatic incident creation and reporting
- **Live Status Reporting**: Real-time device status updates
- **Interactive REPL**: Command-line interface for testing and simulation

## Quick Start

### Installation

```bash
cd apps/client/rust
cargo build --release
```

### Basic Usage

1. **First-time registration**:
   ```bash
   ./target/release/bodycam-client register "Bodycam-001" "site-123"
   ```

2. **Start recording**:
   ```bash
   ./target/release/bodycam-client start --duration 300
   ```

3. **Start recording with incident**:
   ```bash
   ./target/release/bodycam-client start --incident-id "inc-123"
   ```

4. **Stop recording**:
   ```bash
   ./target/release/bodycam-client stop
   ```

5. **Check device status**:
   ```bash
   ./target/release/bodycam-client status
   ```

6. **Trigger incident**:
   ```bash
   ./target/release/bodycam-client trigger-incident --incident-type "emergency" --severity "high"
   ```

## Configuration

The client uses a `config.toml` file for all settings. Copy the example and customize as needed:

```bash
cp src/hardware/config.example.toml config.toml
```

### Key Configuration Sections

#### Simulation Mode (macOS/Testing)
```toml
[simulation]
enabled = true
auto_incidents = true
incident_frequency = 60
```

#### Hardware Configuration (Linux)
```toml
[hardware]
camera_index = 0
microphone = true
gps = true
battery_capacity = 4000
```

#### Recording Settings
```toml
[recording]
resolution = "1920x1080"
fps = 30
bitrate = 5000000
segment_duration = 300
encryption = true
```

## Hardware Setup (Linux)

### GPIO Configuration

The client supports GPIO pins for hardware buttons and LEDs. Configure in `hardware.toml`:

```toml
[gpio]
enabled = true

[[gpio.pins]]
number = 17
direction = "Output"
function = { Led = "Recording" }

[[gpio.pins]]
number = 19
direction = "Input"
function = { Button = "Record" }
```

### Required Hardware

- **Camera**: `/dev/video0` (configurable)
- **Audio**: ALSA device (configurable)
- **GPIO**: Raspberry Pi or similar Linux board
- **Storage**: Minimum 1GB free space

## Simulation Mode

Perfect for development and testing on macOS or without hardware:

```bash
# Enable simulation in config.toml
[simulation]
enabled = true

# Start interactive simulation
./target/release/bodycam-client simulate
```

### Simulation Commands

In simulation mode, use these commands:

- `help` - Show all available commands
- `status` - Show device status
- `press record` - Simulate button press
- `longpress emergency` - Simulate long press
- `motion 5.0` - Simulate motion detection
- `battery 15` - Set battery level to 15%
- `record` - Start recording
- `incident emergency high` - Trigger incident
- `exit` - Exit simulation

## API Integration

The client integrates with the PatrolSight backend API for:

- Device registration and provisioning
- Real-time status reporting
- Incident creation and management
- Video segment uploads
- Authentication and authorization

## Development

### Building from Source

```bash
# Clone the repository
cd apps/client/rust

# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
cargo run -- --verbose status
```

### Project Structure

```
src/
├── main.rs           # CLI entry point
├── device.rs         # Main device implementation
├── auth.rs           # Authentication and provisioning
├── media.rs          # Recording and media handling
├── hardware/         # Hardware abstraction layer
│   ├── mod.rs        # Hardware interface definitions
│   ├── linux.rs      # Linux GPIO/hardware implementation
│   └── macos.rs      # macOS simulation implementation
├── status.rs         # Status reporting and health checks
├── incident.rs       # Incident management
├── simulation/       # Interactive simulation REPL
│   └── mod.rs        # Simulation commands and interface
└── config.rs         # Configuration management
```

### Adding New Hardware Support

To add support for new hardware platforms:

1. Create a new module in `src/hardware/`
2. Implement the `HardwareInterface` trait
3. Update the `create_hardware_interface()` function
4. Add platform-specific configuration in `hardware.toml`

## Troubleshooting

### Common Issues

**"Device not provisioned" error**:
```bash
./target/release/bodycam-client register "device-name" "site-id"
```

**Permission denied on GPIO**:
```bash
# Run as root or add user to gpio group
sudo usermod -a -G gpio $USER
```

**Camera not found**:
```bash
# Check available cameras
ls /dev/video*
# Test with ffmpeg
ffmpeg -f v4l2 -list_formats all -i /dev/video0
```

**Build failures**:
```bash
# Update dependencies
cargo update
# Clean build
cargo clean && cargo build
```

## License

This project is part of the PatrolSight security monitoring platform.
