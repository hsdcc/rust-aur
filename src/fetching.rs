use std::error::Error;
use std::process::Command as Shell;

// --- helper to get installed AUR packages and their installed versions ---
// returns Vec<(name, version_string)>
pub fn get_installed_aur() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let output = Shell::new("pacman").arg("-Qm").output()?;
    if !output.status.success() {
        return Err("failed to run 'pacman -Qm'".into());
    }
    let aur_pkgs = String::from_utf8_lossy(&output.stdout);
    let vec = aur_pkgs
        .lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            match (it.next(), it.next()) {
                (Some(name), Some(ver)) => Some((name.to_string(), ver.to_string())),
                _ => None,
            }
        })
        .collect();
    Ok(vec)
}

// Return true for packages that are debug variants and should be ignored
pub fn is_debug_package(name: &str) -> bool {
    let s = name.to_lowercase();
    s.ends_with("-debug") ||
        s.ends_with("-dbg") ||
        s.ends_with("-dbgsym") ||
        s.ends_with("-debuginfo")
}