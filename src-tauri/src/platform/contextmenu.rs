//! Explorer right-click verb "File Chameleon ile dönüştür" / "Convert with File Chameleon"
//! (opt-in, Settings > General). It lives under `HKCU\Software\Classes\*\shell`, so no admin
//! rights are needed, and runs `"<exe>" --wheel "<file>"`. Explorer starts one process per
//! selected file; the single-instance plugin forwards them and the shell collects the paths
//! for a moment, then opens one wheel in click mode (`windows::queue_wheel_paths`).
//! The uninstaller removes the key (`windows/hooks.nsh`).

use std::path::Path;

/// Command-line switch: the paths after it open the wheel.
pub const WHEEL_ARG: &str = "--wheel";

#[cfg(windows)]
const KEY: &str = r"Software\Classes\*\shell\FileChameleon";

/// `"<exe>" --wheel "%1"`
pub fn command_line(exe: &Path) -> String {
    format!("\"{}\" {WHEEL_ARG} \"%1\"", exe.to_string_lossy())
}

/// The paths after `--wheel` in a command line (empty when the switch is missing).
pub fn wheel_paths(args: &[String]) -> Vec<String> {
    args.iter()
        .skip_while(|a| *a != WHEEL_ARG)
        .skip(1)
        .filter(|a| !a.trim().is_empty())
        .cloned()
        .collect()
}

/// Menu text in the UI language ("tr" | "en").
fn label(lang: &str) -> &'static str {
    if lang == "tr" {
        "File Chameleon ile dönüştür"
    } else {
        "Convert with File Chameleon"
    }
}

/// Write (or remove) the verb. Writing again refreshes the label and the exe path, which fixes
/// a moved or reinstalled exe.
pub fn set_enabled(enabled: bool, lang: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        if enabled {
            let exe = std::env::current_exe()?;
            let icon = format!("\"{}\",0", exe.to_string_lossy());
            reg::set_string(KEY, "MUIVerb", label(lang))?;
            reg::set_string(KEY, "Icon", &icon)?;
            reg::set_string(&format!(r"{KEY}\command"), "", &command_line(&exe))
        } else {
            reg::delete_tree(KEY)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (enabled, label(lang));
        Ok(())
    }
}

#[cfg(windows)]
mod reg {
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RegDeleteTreeW, RegSetKeyValueW,
    };
    use windows::core::{HSTRING, PCWSTR};

    fn check(e: WIN32_ERROR) -> std::io::Result<()> {
        if e.is_ok() {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(e.0 as i32))
        }
    }

    /// `HKCU\<key>` value `name` ("" = the default value) = `value`; creates the key.
    pub fn set_string(key: &str, name: &str, value: &str) -> std::io::Result<()> {
        let data: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        let key = HSTRING::from(key);
        let name = HSTRING::from(name);
        let name = if name.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(name.as_ptr())
        };
        check(unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                name,
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        })
    }

    /// Delete `HKCU\<key>` with its subkeys; a missing key is fine.
    pub fn delete_tree(key: &str) -> std::io::Result<()> {
        let e = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(key)) };
        if e == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_the_exe_and_the_file() {
        assert_eq!(
            command_line(Path::new(
                r"C:\Users\Zeynep Ak\AppData\Local\File Chameleon\file-chameleon.exe"
            )),
            r#""C:\Users\Zeynep Ak\AppData\Local\File Chameleon\file-chameleon.exe" --wheel "%1""#
        );
    }

    #[test]
    fn wheel_paths_are_the_arguments_after_the_switch() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            wheel_paths(&args(&["fc.exe", "--wheel", r"C:\a b\x.png"])),
            [r"C:\a b\x.png"]
        );
        assert_eq!(
            wheel_paths(&args(&[
                "fc.exe",
                "--background",
                "--wheel",
                "a.png",
                "b.jpg"
            ])),
            ["a.png", "b.jpg"]
        );
        assert!(wheel_paths(&args(&["fc.exe", "--background"])).is_empty());
        assert!(wheel_paths(&args(&["fc.exe", "--wheel"])).is_empty());
        // Blank arguments (an empty %1) are dropped.
        assert_eq!(
            wheel_paths(&args(&["fc.exe", "--wheel", "", "a.png"])),
            ["a.png"]
        );
        assert!(wheel_paths(&[]).is_empty());
    }

    #[test]
    fn label_follows_the_language() {
        assert_eq!(label("tr"), "File Chameleon ile dönüştür");
        assert_eq!(label("en"), "Convert with File Chameleon");
    }
}
