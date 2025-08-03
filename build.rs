use std::process::Command;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Get build date
    let build_date = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    // Get git commit
    let git_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    // Get git branch
    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    // Get build target
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    // Create version info file
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("version_info.rs");
    
    let version_info = format!(
        r#"
pub const BUILD_DATE: &str = "{}";
pub const GIT_COMMIT: &str = "{}";
pub const GIT_BRANCH: &str = "{}";
pub const TARGET: &str = "{}";
pub const VERSION_STRING: &str = concat!(env!("CARGO_PKG_VERSION"), "-", "{}");
        "#,
        build_date.trim(),
        git_commit.trim(), 
        git_branch.trim(),
        target,
        git_commit.trim()
    );

    fs::write(dest_path, version_info).unwrap();

    // Re-run if git changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}