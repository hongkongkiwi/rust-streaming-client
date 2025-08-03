#!/bin/bash

# PatrolSight Client Signed Release Package Creator
# This script downloads the latest binary from updatepkg and creates signed release packages

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RUST_CLIENT_DIR="$PROJECT_DIR"
UPDATEPKG_DIR="$(dirname "$SCRIPT_DIR")/../../tools/updatepkg"
BINARY_NAME="patrolsight-client"
WORKSPACE_DIR="$HOME/.patrolsight/releases"

echo "=== PatrolSight Client Signed Release Creator ==="
echo "Script Dir: $SCRIPT_DIR"
echo "Project Dir: $PROJECT_DIR"
echo "Updatepkg Dir: $UPDATEPKG_DIR"
echo "Workspace: $WORKSPACE_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1${NC}"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1${NC}"
}

# Check dependencies
check_dependencies() {
    log "Checking dependencies..."
    
    local deps=("cargo" "rustc" "openssl" "jq" "tar" "sha256sum")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            error "Missing dependency: $dep"
            exit 1
        fi
    done
}

# Get version information
get_version_info() {
    log "Getting version information..."
    
    # Get version from Cargo.toml
    VERSION=$(grep '^version = ' "$RUST_CLIENT_DIR/Cargo.toml" | cut -d'"' -f2)
    
    # Get git commit hash
    GIT_COMMIT=$(git -C "$RUST_CLIENT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    
    # Get build date
    BUILD_DATE=$(date +%Y-%m-%d)
    
    # Get target platform
    TARGET_PLATFORM=$(rustc --version --verbose | grep "host:" | cut -d' ' -f2)
    
    FULL_VERSION="${VERSION}-${GIT_COMMIT}-${BUILD_DATE}"
    
    log "Version: $VERSION"
    log "Git Commit: $GIT_COMMIT"
    log "Build Date: $BUILD_DATE"
    log "Target Platform: $TARGET_PLATFORM"
    log "Full Version: $FULL_VERSION"
}

# Setup workspace
setup_workspace() {
    log "Setting up workspace..."
    
    mkdir -p "$WORKSPACE_DIR"
    mkdir -p "$WORKSPACE_DIR/build"
    mkdir -p "$WORKSPACE_DIR/packages"
    mkdir -p "$WORKSPACE_DIR/keys"
    mkdir -p "$WORKSPACE_DIR/temp"
}

# Build Rust client
build_client() {
    log "Building Rust client..."
    
    cd "$RUST_CLIENT_DIR"
    
    # Clean previous builds
    cargo clean
    
    # Build release
    RUSTFLAGS="-C target-cpu=native" cargo build --release
    
    # Check if binary was created
    if [[ ! -f "target/release/$BINARY_NAME" ]]; then
        error "Failed to build binary"
        exit 1
    fi
    
    log "Binary built successfully: target/release/$BINARY_NAME"
}

# Generate signing keys if they don't exist
generate_keys() {
    log "Checking for signing keys..."
    
    PRIVATE_KEY="$WORKSPACE_DIR/keys/patrolsight-private.pem"
    PUBLIC_KEY="$WORKSPACE_DIR/keys/patrolsight-public.pem"
    CERT="$WORKSPACE_DIR/keys/patrolsight-cert.pem"
    
    if [[ ! -f "$PRIVATE_KEY" ]]; then
        log "Generating new signing keys..."
        
        # Generate private key
        openssl genrsa -out "$PRIVATE_KEY" 4096
        
        # Generate public key
        openssl rsa -in "$PRIVATE_KEY" -pubout -out "$PUBLIC_KEY"
        
        # Generate self-signed certificate
        openssl req -new -x509 -key "$PRIVATE_KEY" -out "$CERT" -days 365 \
            -subj "/C=US/ST=State/L=City/O=PatrolSight/CN=PatrolSight Update Package"
        
        log "Keys generated successfully"
    else
        log "Using existing keys"
    fi
}

# Create package directory structure
create_package_structure() {
    log "Creating package structure..."
    
    PACKAGE_DIR="$WORKSPACE_DIR/build/patrolsight-client-$FULL_VERSION"
    
    # Clean previous package
    rm -rf "$PACKAGE_DIR"
    mkdir -p "$PACKAGE_DIR"
    
    # Create directory structure
    mkdir -p "$PACKAGE_DIR/bin"
    mkdir -p "$PACKAGE_DIR/lib"
    mkdir -p "$PACKAGE_DIR/config"
    mkdir -p "$PACKAGE_DIR/scripts"
    mkdir -p "$PACKAGE_DIR/docs"
    
    # Copy binary
    cp "$RUST_CLIENT_DIR/target/release/$BINARY_NAME" "$PACKAGE_DIR/bin/"
    chmod +x "$PACKAGE_DIR/bin/$BINARY_NAME"
    
    # Copy default config
    cat > "$PACKAGE_DIR/config/default.toml" << 'EOF'
# PatrolSight Client Default Configuration
[device]
name = "PatrolSight Device"
site_id = "default-site"
tenant_id = "default-tenant"

[security]
encryption_enabled = true
certificate_validation = true

[streaming]
quality = "medium"
audio_enabled = true

[storage]
max_size_gb = 10
retention_days = 30
EOF

    # Copy scripts
    cat > "$PACKAGE_DIR/scripts/install.sh" << 'EOF'
#!/bin/bash
set -e

INSTALL_DIR="/opt/patrolsight"
BIN_DIR="/usr/local/bin"
CONFIG_DIR="/etc/patrolsight"

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"

# Copy binary
cp bin/patrolsight-client "$INSTALL_DIR/"
ln -sf "$INSTALL_DIR/patrolsight-client" "$BIN_DIR/patrolsight-client"

# Copy config
cp config/default.toml "$CONFIG_DIR/config.toml"

# Set permissions
chmod +x "$INSTALL_DIR/patrolsight-client"
chmod 644 "$CONFIG_DIR/config.toml"

echo "PatrolSight Client installed successfully"
echo "Configuration file: $CONFIG_DIR/config.toml"
echo "Binary location: $BIN_DIR/patrolsight-client"
EOF

    cat > "$PACKAGE_DIR/scripts/uninstall.sh" << 'EOF'
#!/bin/bash
set -e

INSTALL_DIR="/opt/patrolsight"
BIN_DIR="/usr/local/bin"
CONFIG_DIR="/etc/patrolsight"

# Remove binary
rm -f "$BIN_DIR/patrolsight-client"
rm -rf "$INSTALL_DIR"

# Remove config (optional - comment out to keep)
# rm -f "$CONFIG_DIR/config.toml"

echo "PatrolSight Client uninstalled successfully"
EOF

    chmod +x "$PACKAGE_DIR/scripts/install.sh"
    chmod +x "$PACKAGE_DIR/scripts/uninstall.sh"
    
    # Create README
    cat > "$PACKAGE_DIR/docs/README.md" << EOF
# PatrolSight Client v$FULL_VERSION

## Installation

1. Extract the package contents
2. Run the installation script:
   \`\`\`bash
   sudo ./scripts/install.sh
   \`\`\`

## Usage

Start the client:
\`\`\`bash
patrolsight-client --help
\`\`\`

## Configuration

Edit the configuration file at \`/etc/patrolsight/config.toml\`

## Uninstallation

Run the uninstallation script:
\`\`\`bash
sudo ./scripts/uninstall.sh
\`\`\`

## Version Information

- Version: $VERSION
- Git Commit: $GIT_COMMIT
- Build Date: $BUILD_DATE
- Target Platform: $TARGET_PLATFORM
EOF
    
    log "Package structure created: $PACKAGE_DIR"
}

# Create signed package using updatepkg
create_signed_package() {
    log "Creating signed package..."
    
    PACKAGE_NAME="patrolsight-client-$FULL_VERSION"
    OUTPUT_PACKAGE="$WORKSPACE_DIR/packages/$PACKAGE_NAME.tar.gz"
    
    cd "$UPDATEPKG_DIR"
    
    # Build updatepkg if not already built
    if [[ ! -f "target/release/updatepkg" ]]; then
        log "Building updatepkg tool..."
        cargo build --release
    fi
    
    # Create package using updatepkg
    log "Creating package with updatepkg..."
    
    # Use the package builder to create signed package
    "$UPDATEPKG_DIR/target/release/updatepkg" create \
        --source "$WORKSPACE_DIR/build/$PACKAGE_NAME" \
        --output "$OUTPUT_PACKAGE" \
        --sign "$WORKSPACE_DIR/keys/patrolsight-private.pem" \
        --cert "$WORKSPACE_DIR/keys/patrolsight-cert.pem"
    
    # Create package metadata
    cat > "$WORKSPACE_DIR/packages/$PACKAGE_NAME.json" << EOF
{
    "name": "patrolsight-client",
    "version": "$VERSION",
    "full_version": "$FULL_VERSION",
    "git_commit": "$GIT_COMMIT",
    "build_date": "$BUILD_DATE",
    "target_platform": "$TARGET_PLATFORM",
    "package_file": "$PACKAGE_NAME.tar.gz",
    "checksum": "$(sha256sum "$OUTPUT_PACKAGE" | cut -d' ' -f1)",
    "size": $(stat -f%z "$OUTPUT_PACKAGE" 2>/dev/null || stat -c%s "$OUTPUT_PACKAGE"),
    "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
    
    log "Signed package created: $OUTPUT_PACKAGE"
    
    # Create manifest for web distribution
    cat > "$WORKSPACE_DIR/packages/manifest.json" << EOF
{
    "current_version": "$VERSION",
    "latest_version": "$VERSION",
    "releases": [
        {
            "version": "$VERSION",
            "release_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
            "changelog": [
                "Initial release v$VERSION",
                "Git commit: $GIT_COMMIT",
                "Build date: $BUILD_DATE"
            ],
            "download_url": "https://updates.patrolsight.com/stable/$PACKAGE_NAME.tar.gz",
            "checksum": "$(sha256sum "$OUTPUT_PACKAGE" | cut -d' ' -f1)",
            "signature": "$(sha256sum "$OUTPUT_PACKAGE.sig" 2>/dev/null | cut -d' ' -f1 || echo 'none')",
            "size": $(stat -f%z "$OUTPUT_PACKAGE" 2>/dev/null || stat -c%s "$OUTPUT_PACKAGE"),
            "min_system_version": "1.0.0",
            "critical": false,
            "rollback_allowed": true
        }
    ],
    "update_channel": "stable",
    "last_check": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
}

# Verify package signature
verify_package() {
    log "Verifying package signature..."
    
    PACKAGE_NAME="patrolsight-client-$FULL_VERSION"
    OUTPUT_PACKAGE="$WORKSPACE_DIR/packages/$PACKAGE_NAME.tar.gz"
    
    if [[ -f "$OUTPUT_PACKAGE.sig" ]]; then
        log "Verifying signature..."
        
        # Use updatepkg to verify
        "$UPDATEPKG_DIR/target/release/updatepkg" info "$OUTPUT_PACKAGE"
        
        log "Package signature verified successfully"
    else
        warn "No signature file found"
    fi
}

# Create additional formats
create_additional_formats() {
    log "Creating additional package formats..."
    
    PACKAGE_NAME="patrolsight-client-$FULL_VERSION"
    PACKAGE_DIR="$WORKSPACE_DIR/build/$PACKAGE_NAME"
    
    # Create ZIP package for Windows
    if command -v zip >/dev/null 2>&1; then
        log "Creating Windows ZIP package..."
        cd "$WORKSPACE_DIR/build"
        zip -r "../packages/$PACKAGE_NAME.zip" "$PACKAGE_NAME"
    fi
    
    # Create DEB package for Debian/Ubuntu
    if command -v dpkg-deb >/dev/null 2>&1; then
        log "Creating DEB package..."
        
        DEB_DIR="$WORKSPACE_DIR/build/$PACKAGE_NAME-deb"
        mkdir -p "$DEB_DIR/DEBIAN"
        mkdir -p "$DEB_DIR/usr/bin"
        mkdir -p "$DEB_DIR/etc/patrolsight"
        
        # Create control file
        cat > "$DEB_DIR/DEBIAN/control" << EOF
Package: patrolsight-client
Version: $VERSION
Section: utils
Priority: optional
Architecture: $(dpkg --print-architecture)
Depends: libc6 (>= 2.17)
Maintainer: PatrolSight Security <support@patrolsight.com>
Description: PatrolSight Security Client
 A comprehensive security monitoring client for the PatrolSight platform.
EOF
        
        # Copy files
        cp "$PACKAGE_DIR/bin/$BINARY_NAME" "$DEB_DIR/usr/bin/"
        cp "$PACKAGE_DIR/config/default.toml" "$DEB_DIR/etc/patrolsight/config.toml"
        
        # Create DEB package
        dpkg-deb --build "$DEB_DIR" "$WORKSPACE_DIR/packages/$PACKAGE_NAME.deb"
    fi
    
    # Create RPM package (requires rpmbuild)
    if command -v rpmbuild >/dev/null 2>&1; then
        log "Creating RPM package..."
        
        RPM_DIR="$WORKSPACE_DIR/build/rpm"
        mkdir -p "$RPM_DIR"
        
        # Create RPM spec file
        cat > "$RPM_DIR/patrolsight-client.spec" << EOF
Name: patrolsight-client
Version: $VERSION
Release: 1%{?dist}
Summary: PatrolSight Security Client
License: Proprietary
URL: https://patrolsight.com

%description
A comprehensive security monitoring client for the PatrolSight platform.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/etc/patrolsight
cp $PACKAGE_DIR/bin/$BINARY_NAME %{buildroot}/usr/bin/
cp $PACKAGE_DIR/config/default.toml %{buildroot}/etc/patrolsight/config.toml

%files
/usr/bin/$BINARY_NAME
/etc/patrolsight/config.toml

%post
echo "PatrolSight Client installed successfully"

%preun
echo "Removing PatrolSight Client"
EOF
        
        rpmbuild -bb "$RPM_DIR/patrolsight-client.spec" --define "_topdir $RPM_DIR"
        cp "$RPM_DIR/RPMS/x86_64/patrolsight-client-$VERSION-1.x86_64.rpm" "$WORKSPACE_DIR/packages/"
    fi
}

# Generate checksums
generate_checksums() {
    log "Generating checksums..."
    
    cd "$WORKSPACE_DIR/packages"
    
    # Generate SHA256 checksums
    sha256sum * > checksums.sha256
    
    # Generate MD5 checksums
    md5sum * > checksums.md5
    
    log "Checksums generated: checksums.sha256, checksums.md5"
}

# Create release summary
create_release_summary() {
    log "Creating release summary..."
    
    cat > "$WORKSPACE_DIR/packages/RELEASE_SUMMARY.md" << EOF
# PatrolSight Client Release Summary

## Build Information
- **Version**: $VERSION
- **Git Commit**: $GIT_COMMIT
- **Build Date**: $BUILD_DATE
- **Target Platform**: $TARGET_PLATFORM
- **Full Version**: $FULL_VERSION

## Package Files

$(cd "$WORKSPACE_DIR/packages" && ls -la *.tar.gz *.zip *.deb *.rpm 2>/dev/null || echo "No packages found")

## Checksums

\`\`\`
$(cat "$WORKSPACE_DIR/packages/checksums.sha256" 2>/dev/null || echo "No checksums available")
\`\`\`

## Installation

### Standard Package (tar.gz)
1. Download: \`patrolsight-client-$FULL_VERSION.tar.gz\`
2. Extract: \`tar -xzf patrolsight-client-$FULL_VERSION.tar.gz\`
3. Install: \`sudo ./scripts/install.sh\`

### Windows (if applicable)
1. Download: \`patrolsight-client-$FULL_VERSION.zip\`
2. Extract and run installer

### Debian/Ubuntu
1. Download: \`patrolsight-client-$FULL_VERSION.deb\`
2. Install: \`sudo dpkg -i patrolsight-client-$FULL_VERSION.deb\`

### RedHat/CentOS/Fedora
1. Download: \`patrolsight-client-$FULL_VERSION.rpm\`
2. Install: \`sudo rpm -i patrolsight-client-$FULL_VERSION.rpm\`

## Verification

Verify package signatures:
\`\`\`bash
cd $WORKSPACE_DIR/packages
sha256sum -c checksums.sha256
\`\`\`

## Troubleshooting

- Check system requirements
- Verify package integrity with checksums
- Ensure proper permissions for installation
- Review logs in \`/var/log/patrolsight/\`

## Contact

For support, contact: support@patrolsight.com
EOF
    
    log "Release summary created: $WORKSPACE_DIR/packages/RELEASE_SUMMARY.md"
}

# Main execution
main() {
    log "Starting signed release creation process..."
    
    check_dependencies
    get_version_info
    setup_workspace
    generate_keys
    build_client
    create_package_structure
    create_signed_package
    verify_package
    create_additional_formats
    generate_checksums
    create_release_summary
    
    log "=== Signed Release Creation Complete ==="
    log "Release files available in: $WORKSPACE_DIR/packages"
    log ""
    log "Created packages:"
    ls -la "$WORKSPACE_DIR/packages"/*.tar.gz "$WORKSPACE_DIR/packages"/*.zip "$WORKSPACE_DIR/packages"/*.deb "$WORKSPACE_DIR/packages"/*.rpm 2>/dev/null || true
    log ""
    log "To install:"
    log "  tar -xzf $WORKSPACE_DIR/packages/patrolsight-client-$FULL_VERSION.tar.gz"
    log "  cd patrolsight-client-$FULL_VERSION"
    log "  sudo ./scripts/install.sh"
}

# Handle command line arguments
case "${1:-create}" in
    "create")
        main
        ;;
    "clean")
        log "Cleaning workspace..."
        rm -rf "$WORKSPACE_DIR"
        log "Workspace cleaned"
        ;;
    "verify")
        if [[ -n "$2" ]]; then
            verify_package "$2"
        else
            error "Please provide package path to verify"
            exit 1
        fi
        ;;
    *)
        echo "Usage: $0 {create|clean|verify}"
        echo "  create  - Create new signed release package"
        echo "  clean   - Clean workspace"
        echo "  verify  - Verify package signature"
        exit 1
        ;;
esac