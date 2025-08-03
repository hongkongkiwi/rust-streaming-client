#!/bin/bash

# PatrolSight Client Update Script
# Similar to updatepkg functionality

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY_NAME="patrolsight-client"
BACKUP_DIR="$HOME/.patrolsight/backups"
DOWNLOAD_DIR="$HOME/.patrolsight/downloads"
UPDATE_URL="https://updates.patrolsight.com"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
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
    local deps=("curl" "jq" "sha256sum" "tar")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            error "Missing dependency: $dep"
            exit 1
        fi
    done
}

# Get current version
get_current_version() {
    if [[ -f "$PROJECT_DIR/target/release/$BINARY_NAME" ]]; then
        "$PROJECT_DIR/target/release/$BINARY_NAME" --version 2>/dev/null | head -1 || echo "unknown"
    else
        echo "not_installed"
    fi
}

# Check for updates
check_updates() {
    local channel="${1:-stable}"
    log "Checking for updates on $channel channel..."
    
    local manifest_url="$UPDATE_URL/$channel/manifest.json"
    local temp_file=$(mktemp)
    
    if curl -s "$manifest_url" > "$temp_file"; then
        local latest_version=$(jq -r '.releases[0].version' "$temp_file")
        local download_url=$(jq -r '.releases[0].download_url' "$temp_file")
        local checksum=$(jq -r '.releases[0].checksum' "$temp_file")
        local changelog=$(jq -r '.releases[0].changelog | join("\\n  - ")' "$temp_file")
        
        local current_version=$(get_current_version)
        
        if [[ "$latest_version" != "$current_version" ]]; then
            log "Update available: $current_version -> $latest_version"
            log "Changelog:"
            echo "  - $changelog"
            echo "$latest_version|$download_url|$checksum"
        else
            log "Already up to date: $current_version"
            echo "up_to_date"
        fi
    else
        error "Failed to fetch update manifest"
        exit 1
    fi
    
    rm -f "$temp_file"
}

# Download update
download_update() {
    local download_url="$1"
    local checksum="$2"
    local filename=$(basename "$download_url")
    
    mkdir -p "$DOWNLOAD_DIR"
    local download_path="$DOWNLOAD_DIR/$filename"
    
    log "Downloading update..."
    curl -L -o "$download_path" "$download_url"
    
    log "Verifying checksum..."
    local computed_checksum=$(sha256sum "$download_path" | cut -d' ' -f1)
    
    if [[ "$computed_checksum" != "$checksum" ]]; then
        error "Checksum verification failed!"
        rm -f "$download_path"
        exit 1
    fi
    
    log "Download completed and verified: $download_path"
    echo "$download_path"
}

# Create backup
create_backup() {
    local current_version="$1"
    local backup_name="$(date +%Y%m%d_%H%M%S)_$current_version"
    
    mkdir -p "$BACKUP_DIR"
    
    if [[ -f "$PROJECT_DIR/target/release/$BINARY_NAME" ]]; then
        log "Creating backup: $backup_name"
        cp "$PROJECT_DIR/target/release/$BINARY_NAME" "$BACKUP_DIR/$backup_name"
        echo "$BACKUP_DIR/$backup_name"
    else
        warn "No existing binary to backup"
    fi
}

# Apply update
apply_update() {
    local update_file="$1"
    local backup_path="$2"
    
    log "Applying update..."
    
    # Stop running processes
    pkill -f "$BINARY_NAME" || true
    sleep 2
    
    # Extract and replace binary
    if [[ "$update_file" == *.tar.gz ]]; then
        tar -xzf "$update_file" -C "$PROJECT_DIR/target/release/"
    else
        cp "$update_file" "$PROJECT_DIR/target/release/$BINARY_NAME"
        chmod +x "$PROJECT_DIR/target/release/$BINARY_NAME"
    fi
    
    # Verify installation
    if [[ -f "$PROJECT_DIR/target/release/$BINARY_NAME" ]]; then
        local new_version=$(get_current_version)
        log "Update applied successfully: $new_version"
    else
        error "Update failed - binary not found"
        if [[ -n "$backup_path" ]]; then
            log "Restoring from backup..."
            cp "$backup_path" "$PROJECT_DIR/target/release/$BINARY_NAME"
        fi
        exit 1
    fi
}

# Rollback to previous version
rollback() {
    log "Finding latest backup..."
    
    local latest_backup=$(ls -t "$BACKUP_DIR"/*/"$BINARY_NAME" 2>/dev/null | head -1)
    
    if [[ -z "$latest_backup" ]]; then
        error "No backup found for rollback"
        exit 1
    fi
    
    log "Rolling back to: $(basename "$latest_backup")"
    
    # Stop running processes
    pkill -f "$BINARY_NAME" || true
    sleep 2
    
    # Restore from backup
    cp "$latest_backup" "$PROJECT_DIR/target/release/$BINARY_NAME"
    chmod +x "$PROJECT_DIR/target/release/$BINARY_NAME"
    
    log "Rollback completed"
}

# Build from source
build_from_source() {
    log "Building from source..."
    
    cd "$PROJECT_DIR"
    
    # Clean previous builds
    cargo clean
    
    # Build release
    cargo build --release
    
    if [[ -f "target/release/$BINARY_NAME" ]]; then
        log "Build completed successfully"
    else
        error "Build failed"
        exit 1
    fi
}

# Main update process
main() {
    check_dependencies
    
    local command="${1:-check}"
    local channel="${2:-stable}"
    
    case "$command" in
        "check")
            check_updates "$channel"
            ;;
        "update")
            log "Starting update process..."
            
            local update_info=$(check_updates "$channel")
            if [[ "$update_info" == "up_to_date" ]]; then
                log "Already up to date"
                exit 0
            fi
            
            local IFS='|'
            read -r latest_version download_url checksum <<< "$update_info"
            
            local current_version=$(get_current_version)
            local backup_path=$(create_backup "$current_version")
            
            local update_file=$(download_update "$download_url" "$checksum")
            apply_update "$update_file" "$backup_path"
            
            log "Update completed successfully!"
            ;;
        "rollback")
            rollback
            ;;
        "build")
            build_from_source
            ;;
        *)
            echo "Usage: $0 {check|update|rollback|build} [channel]"
            echo "  check    - Check for updates"
            echo "  update   - Download and apply updates"
            echo "  rollback - Rollback to previous version"
            echo "  build    - Build from source"
            echo ""
            echo "Channels: stable, beta, alpha, development"
            exit 1
            ;;
    esac
}

# Run main function
main "$@"