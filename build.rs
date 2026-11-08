#[allow(dead_code)]
#[path = "src/presentation.rs"]
mod presentation;

fn main() {
    println!("cargo:rerun-if-changed=src/presentation.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(output) = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
    {
        if output.status.success() {
            let reference = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if let Ok(path) = std::process::Command::new("git")
                .args(["rev-parse", "--git-path", &reference])
                .output()
            {
                if path.status.success() {
                    println!(
                        "cargo:rerun-if-changed={}",
                        String::from_utf8_lossy(&path.stdout).trim()
                    );
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=assets/branding/sports-route.ico");
    println!("cargo:rerun-if-env-changed=NSWT_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=NSWT_BUILD_TARGET");
    let commit = std::env::var("NSWT_BUILD_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .unwrap_or_else(|| "development".to_owned());
    let target = std::env::var("NSWT_BUILD_TARGET")
        .or_else(|_| std::env::var("TARGET"))
        .unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=NSWT_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=NSWT_BUILD_TARGET={target}");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
    }
}

#[cfg(windows)]
fn windows_resources() {
    use winresource::{VersionInfo, WindowsResource};

    let components: Vec<u16> = presentation::VERSION
        .split('.')
        .map(|value| value.parse().expect("numeric release version component"))
        .collect();
    assert_eq!(
        components.len(),
        3,
        "release version must have three components"
    );
    let numeric = (u64::from(components[0]) << 48)
        | (u64::from(components[1]) << 32)
        | (u64::from(components[2]) << 16);
    let mut resources = WindowsResource::new();
    resources
        .set_icon("assets/branding/sports-route.ico")
        .set("ProductName", presentation::APP_NAME)
        .set("FileDescription", presentation::APP_NAME)
        .set("Comments", presentation::SIGNATURE)
        .set("InternalName", presentation::APP_NAME)
        .set("OriginalFilename", "NekoSportsWorldTool.exe")
        .set(
            "LegalCopyright",
            "Copyright (c) 2026 YanamiNeko. CC BY-NC 4.0.",
        )
        .set("FileVersion", presentation::VERSION)
        .set("ProductVersion", presentation::VERSION)
        .set_version_info(VersionInfo::FILEVERSION, numeric)
        .set_version_info(VersionInfo::PRODUCTVERSION, numeric)
        .set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"><security><requestedPrivileges>
    <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
  </requestedPrivileges></security></trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
    <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
  </application></compatibility>
</assembly>"#,
        )
        .compile_for("nekosportsworldtool")
        .expect("compile Windows icon/version resources");
}
