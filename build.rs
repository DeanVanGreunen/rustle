use std::process::Command;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/logo.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed logo.ico: {e}");
        }
    }
    let date = if cfg!(windows) {
        // PowerShell is the reliable way to get an ISO date on Windows
        Command::new("powershell")
            .args(["-Command", "Get-Date -Format yyyy-MM-dd"])
            .output()
    } else {
        Command::new("date").args(["+%Y-%m-%d"]).output()
    }
    .expect("failed to get build date");
    println!("cargo:rustc-env=BUILD_DATE={}", String::from_utf8_lossy(&date.stdout).trim());
    if let Ok(iter) = dotenvy::dotenv_iter() {
        for item in iter {
            if let Ok((key, val)) = item {
                println!("cargo:rustc-env={key}={val}");
            }
        }
    }
    let commit = Command::new("git")
    .args(["rev-parse", "--short", "HEAD"])
    .output()
    .ok()
    .filter(|o| o.status.success())
    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    .unwrap_or_else(|| "Invalid Build".into());
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rustc-env=BUILD_ID={commit}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=NONEXISTENT_FILE_TO_ALWAYS_RERUN"); // force fresh id each build
}
