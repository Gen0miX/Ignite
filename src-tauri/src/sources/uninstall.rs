//! Windows "installed programs" list (Uninstall registry keys), read-only.
//! Shared by launchers that register their games there (Riot, Battle.net, EA,
//! Ubisoft).

/// One `...\CurrentVersion\Uninstall\<key>` entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UninstallEntry {
    /// Registry key name (e.g. `Riot Game valorant.live`, `Uplay Install 635`).
    pub key: String,
    pub display_name: String,
    pub publisher: String,
    pub install_location: Option<String>,
    pub display_icon: Option<String>,
    pub uninstall_string: Option<String>,
}

/// Registry paths use `/` sometimes (`C:/Riot Games/...`): normalize.
pub fn clean_path(p: &str) -> String {
    p.trim()
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_string()
}

/// `DisplayIcon` may be `"C:\x\game.exe",0`: keep the path.
pub fn icon_path(icon: &str) -> String {
    let p = icon.trim();
    let p = p
        .rsplit_once(',')
        .filter(|(_, n)| n.trim().parse::<i32>().is_ok())
        .map_or(p, |(a, _)| a);
    clean_path(p)
}

/// Every entry of HKLM (64 and 32 bits) and HKCU.
pub fn read_all() -> Vec<UninstallEntry> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;
        let roots = [
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
        ];
        let mut out = Vec::new();
        for (hive, path) in roots {
            let Ok(root) = RegKey::predef(hive).open_subkey(path) else {
                continue;
            };
            for name in root.enum_keys().flatten() {
                let Ok(k) = root.open_subkey(&name) else {
                    continue;
                };
                let get = |v: &str| {
                    k.get_value::<String, _>(v)
                        .ok()
                        .filter(|s| !s.trim().is_empty())
                };
                let Some(display_name) = get("DisplayName") else {
                    continue;
                };
                out.push(UninstallEntry {
                    key: name,
                    display_name,
                    publisher: get("Publisher").unwrap_or_default(),
                    install_location: get("InstallLocation"),
                    display_icon: get("DisplayIcon"),
                    uninstall_string: get("UninstallString"),
                });
            }
        }
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_paths_and_icons() {
        assert_eq!(
            clean_path("C:/Riot Games/VALORANT/live/"),
            r"C:\Riot Games\VALORANT\live"
        );
        assert_eq!(
            icon_path(r#""C:\Games\D4\Diablo IV.exe",0"#),
            r"C:\Games\D4\Diablo IV.exe"
        );
        assert_eq!(
            icon_path(r"C:\Games\a,b\game.exe"),
            r"C:\Games\a,b\game.exe"
        );
    }
}
