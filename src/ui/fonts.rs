//! System CJK fonts with a bundled, OFL-licensed cross-platform fallback.

const BUNDLED_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSansSC-VF.ttf");

#[cfg(not(target_os = "android"))]
fn system_font() -> Option<(String, Vec<u8>)> {
    #[cfg(target_os = "windows")]
    let candidates: Vec<std::path::PathBuf> = {
        let directory = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)?
            .join("Fonts");
        ["msyh.ttc", "msyh.ttf", "simhei.ttf", "simsun.ttc"]
            .into_iter()
            .map(|name| directory.join(name))
            .collect()
    };
    #[cfg(target_os = "macos")]
    let candidates: Vec<std::path::PathBuf> = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .collect();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let candidates: Vec<std::path::PathBuf> = Vec::new();
    candidates.into_iter().find_map(|path| {
        let name = path.file_name()?.to_string_lossy().into_owned();
        std::fs::read(path).ok().map(|bytes| (name, bytes))
    })
}

/// Installation always retains the embedded font, even if a system font exists.
pub fn install(ctx: &egui::Context) -> Option<String> {
    let name = "Noto Sans SC".to_owned();
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(name.clone(), egui::FontData::from_static(BUNDLED_FONT));
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push(name.clone());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push(name.clone());
    }
    #[allow(unused_mut)]
    let mut selected = name;
    #[cfg(not(target_os = "android"))]
    if let Some((name, bytes)) = system_font() {
        fonts
            .font_data
            .insert(name.clone(), egui::FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            let position = list.len().saturating_sub(1);
            list.insert(position, name.clone());
        }
        if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            let position = list.len().saturating_sub(1);
            list.insert(position, name.clone());
        }
        selected = name;
    }
    ctx.set_fonts(fonts);
    Some(selected)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_fallback_contains_the_chinese_credit() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        let name = "bundled-cjk".to_owned();
        fonts.font_data.insert(
            name.clone(),
            egui::FontData::from_static(super::BUNDLED_FONT),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .push(name);
        ctx.set_fonts(fonts);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts(|fonts| {
                assert!(fonts.has_glyphs(&egui::FontId::proportional(14.0), "田宸宇"))
            });
        });
    }
}
