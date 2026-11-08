//! Platform services shared by the existing UI and the Android host.

#[derive(Clone, Copy, Debug)]
pub enum InputKind {
    Text,
    Password,
    Integer,
    Decimal,
}

pub fn data_dir() -> std::path::PathBuf {
    try_data_dir().expect("application data directory must be validated before use")
}

/// Resolve the user's application data root without creating or modifying it.
pub fn try_data_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(test)]
    if let Some(path) = TEST_DATA_DIR.with(|p| p.borrow().clone()) {
        return Ok(path);
    }
    if let Some(path) = std::env::var_os("NSWT_DATA_DIR") {
        let path = std::path::PathBuf::from(path);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err("NSWT_DATA_DIR 必须是绝对路径".into())
        };
    }
    #[cfg(target_os = "android")]
    {
        Ok(crate::android::data_dir())
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("无法确定用户主目录")?;
        let home = std::path::PathBuf::from(home);
        if !home.is_absolute() {
            return Err("HOME 必须是绝对路径".into());
        }
        Ok(home.join("Library/Application Support/NekoSportsWorldTool-J-claude"))
    }
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var_os("LOCALAPPDATA").ok_or("无法确定 LOCALAPPDATA")?;
        let path = std::path::PathBuf::from(local);
        if !path.is_absolute() {
            return Err("LOCALAPPDATA 必须是绝对路径".into());
        }
        Ok(path.join("NekoSportsWorldTool-J-claude"))
    }
    #[cfg(not(any(target_os = "android", target_os = "macos", target_os = "windows")))]
    {
        Ok(std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .ok_or("无法定位程序目录")?)
    }
}

pub fn edit_text(id: u64, value: &str, kind: InputKind) {
    #[cfg(target_os = "android")]
    crate::android::edit_text(id, value, kind);
    #[cfg(not(target_os = "android"))]
    let _ = (id, value, kind);
}

pub fn take_edited_text(id: u64) -> Option<String> {
    #[cfg(target_os = "android")]
    {
        crate::android::take_edited_text(id)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = id;
        None
    }
}

pub fn set_keep_screen_on(enabled: bool) {
    #[cfg(target_os = "android")]
    crate::android::set_keep_screen_on(enabled);
    #[cfg(not(target_os = "android"))]
    let _ = enabled;
}

/// 当前程序版本：桌面读 Cargo 版本，Android 读 manifest versionName
/// （形如 0.2.5-android.3，与 Release tag 对齐）。
pub fn version_name() -> String {
    #[cfg(target_os = "android")]
    {
        crate::android::version_name()
    }
    #[cfg(not(target_os = "android"))]
    {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

/// Android：把已下载的 APK 交给系统安装器（用户在系统弹窗确认安装）。
pub fn install_apk(path: &str) {
    #[cfg(target_os = "android")]
    crate::android::install_apk(path);
    #[cfg(not(target_os = "android"))]
    let _ = path;
}

#[cfg(feature = "gui")]
pub fn sync_clipboard(context: &egui::Context) {
    #[cfg(target_os = "android")]
    {
        let text = context.output(|output| output.copied_text.clone());
        if !text.is_empty() {
            crate::android::copy_text(&text);
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = context;
}

#[cfg(feature = "gui")]
pub fn apply_safe_area(context: &egui::Context, input: &mut egui::RawInput) {
    #[cfg(target_os = "android")]
    if let Some(rect) = input.screen_rect.as_mut() {
        let [left, top, right, bottom] = crate::android::safe_insets();
        let scale = context.pixels_per_point();
        rect.min.x += left as f32 / scale;
        rect.min.y += top as f32 / scale;
        rect.max.x -= right as f32 / scale;
        rect.max.y -= bottom as f32 / scale;
    }
    #[cfg(not(target_os = "android"))]
    let _ = (context, input);
}

#[cfg(test)]
thread_local! {
    pub(crate) static TEST_DATA_DIR: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}
