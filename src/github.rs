use raur_lib::GITHUB_AUR_MIRROR_RAW_BASE;
use reqwest::StatusCode;
use reqwest::blocking::get;
use std::error::Error;
use std::process::Command as Shell;

pub enum PkgbuildState {
    Result(String),
    NotFound,
    OtherError,
}

// Fetch PKGBUILD from the GitHub aur mirror branch for package `pkg`
// (raw URL: https://raw.githubusercontent.com/archlinux/aur/<branch>/PKGBUILD)
pub fn fetch_pkgbuild_from_github(pkg: &str) -> Result<PkgbuildState, Box<dyn Error>> {
    let url = format!("{}/{}/PKGBUILD", GITHUB_AUR_MIRROR_RAW_BASE, pkg);
    let resp = get(&url)?;
    if !resp.status().is_success() {
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(PkgbuildState::NotFound);
        }
        return Ok(PkgbuildState::OtherError);
    }
    let body = resp.text()?;
    Ok(PkgbuildState::Result(body))
}

// Parse pkgver and pkgrel from PKGBUILD text.
// Returns combined version string like "1.2.3-4" (pkgver-pkgrel), or just "1.2.3" if pkgrel missing.
pub fn parse_pkgbuild_version(build: &str) -> Option<String> {
    // naive but practical parsing:
    // look for lines like: pkgver=1.2.3 or pkgver='1.2.3' or pkgver="1.2.3"
    // and pkgrel=4 or pkgrel='4'
    let mut pkgver: Option<String> = None;
    let mut pkgrel: Option<String> = None;

    for line in build.lines() {
        let l = line.trim();
        // ignore comments
        if l.starts_with('#') {
            continue;
        }
        if l.starts_with("pkgver") && l.contains('=') {
            if let Some(idx) = l.find('=') {
                let mut val = l[idx + 1..].trim();
                // strip quotes
                if
                    (val.starts_with('\'') && val.ends_with('\'')) ||
                    (val.starts_with('"') && val.ends_with('"'))
                {
                    val = &val[1..val.len() - 1];
                }
                // ignore complex assignments (like pkgver=$(git describe ...))
                if !val.contains('$') && !val.contains('(') {
                    pkgver = Some(val.to_string());
                } else {
                    // complicated pkgver; bail out (cannot parse reliably)
                    return None;
                }
            }
        } else if l.starts_with("pkgrel") && l.contains('=') {
            if let Some(idx) = l.find('=') {
                let mut val = l[idx + 1..].trim();
                if
                    (val.starts_with('\'') && val.ends_with('\'')) ||
                    (val.starts_with('"') && val.ends_with('"'))
                {
                    val = &val[1..val.len() - 1];
                }
                if !val.contains('$') && !val.contains('(') {
                    pkgrel = Some(val.to_string());
                } else {
                    // complicated pkgrel; bail out
                    return None;
                }
            }
        }
        // stop early if both found
        if pkgver.is_some() && pkgrel.is_some() {
            break;
        }
    }

    match (pkgver, pkgrel) {
        (Some(v), Some(r)) => Some(format!("{}-{}", v, r)),
        (Some(v), None) => Some(v),
        _ => None,
    }
}

pub fn fetch_github_packages() -> Result<Vec<String>, Box<dyn Error>> {
    let output = Shell::new("git")
        .arg("ls-remote")
        .arg("--heads")
        .arg("https://github.com/archlinux/aur.git")
        .output()?;

    if !output.status.success() {
        return Err(format!("git ls-remote failed with status: {}", output.status).into());
    }

    let data = String::from_utf8_lossy(&output.stdout);
    let packages = data
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|s| s.strip_prefix("refs/heads/").unwrap_or(s).to_string())
        .collect();
    Ok(packages)
}

pub fn github_package_exists(pkg: &str, list: &[String]) -> bool {
    list.contains(&pkg.to_string())
}
