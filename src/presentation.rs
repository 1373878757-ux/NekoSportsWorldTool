//! Single source of truth for the application's identity and customization credit.
//! The contributor's name is a complete credit, never a separate product brand.

pub const APP_NAME: &str = "NekoSportsWorldTool";
pub const TAGLINE: &str = "just callme...";
pub const SIGNATURE: &str = "prod.by 田宸宇 J Claude";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const UPSTREAM_AUTHOR: &str = "YanamiNeko";
pub const UPSTREAM_REPOSITORY: &str = "YanamiNeko/NekoSportsWorldTool";
pub const SOURCE_REPOSITORY: &str = "ConCosew/NekoSportsWorldTool";
pub const LICENSE_NAME: &str = "CC BY-NC 4.0";
pub const CUSTOMIZATION_NOTE: &str = "基于上游项目的界面署名定制与跨平台发行工程修改";
pub const BUILD_COMMIT: &str = match option_env!("NSWT_BUILD_COMMIT") {
    Some(value) => value,
    None => "development",
};
pub const BUILD_TARGET: &str = match option_env!("NSWT_BUILD_TARGET") {
    Some(value) => value,
    None => std::env::consts::OS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_text_matches_the_approved_spelling_exactly() {
        assert_eq!(APP_NAME, "NekoSportsWorldTool");
        assert_eq!(TAGLINE, "just callme...");
        assert_eq!(SIGNATURE, "prod.by 田宸宇 J Claude");
        assert_eq!(SIGNATURE.matches("田宸宇").count(), 1);
        assert!(!SIGNATURE.contains("J claude"));
    }

    #[test]
    fn gui_files_do_not_duplicate_the_approved_presentation_literals() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut sources = vec![root.join("src/main.rs")];
        for entry in std::fs::read_dir(root.join("src/ui")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
        for path in sources {
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(
                !source.contains(TAGLINE),
                "{} duplicates TAGLINE",
                path.display()
            );
            assert!(
                !source.contains(SIGNATURE),
                "{} duplicates SIGNATURE",
                path.display()
            );
        }
    }
}
