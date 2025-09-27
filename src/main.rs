// main.rs - Simple AUR Helper
use clap::{ Arg, ArgAction, Command };
use reqwest::blocking::get;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::io::{ self, Write };
use std::process::Command as Shell;

const AUR_RPC: &str = "https://aur.archlinux.org/rpc/?v=5&";
const GITHUB_AUR_MIRROR_RAW_BASE: &str = "https://raw.githubusercontent.com/archlinux/aur";

const YES_OPTIONS: [&'static str; 4] = ["y", "yes", "true", "yeah"];
const NO_OPTIONS: [&'static str; 4] = ["n", "not", "no", "nope"];

#[derive(Deserialize)]
struct RpcResponse {
    results: Vec<AurPkg>,
}

#[derive(Deserialize, Clone)]
struct AurPkg {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: Option<String>,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Popularity")]
    popularity: Option<f32>,
    #[serde(rename = "Maintainer")]
    maintainer: Option<String>,
    #[serde(rename = "Depends")]
    #[serde(default)]
    depends: Vec<String>,
    #[serde(rename = "MakeDepends")]
    #[serde(default)]
    make_depends: Vec<String>,
}

// simple yes/no prompt
fn prompt_yes(question: &str) -> bool {
    print!("{} [Y/n] ", question);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let resp = input.trim().to_lowercase();

    if YES_OPTIONS.contains(&resp.as_str()) {
        return true;
    } else if NO_OPTIONS.contains(&resp.as_str()) {
        return false;
    } else {
        return prompt_yes(question);
    }
}

// --- AUR RPC helpers ---
fn fetch_search(term: &str) -> Result<Vec<AurPkg>, Box<dyn Error>> {
    let url = format!("{}type=search&arg={}", AUR_RPC, term);
    let resp: RpcResponse = get(&url)?.json()?;
    let mut packages = resp.results;
    packages.sort_by(|a, b| {
        b.popularity.unwrap_or(0.0).partial_cmp(&a.popularity.unwrap_or(0.0)).unwrap()
    });
    Ok(packages)
}

fn fetch_info(name: &str) -> Result<AurPkg, Box<dyn Error>> {
    let url = format!("{}type=info&arg={}", AUR_RPC, name);
    let resp: RpcResponse = get(&url)?.json()?;
    resp.results
        .into_iter()
        .next()
        .ok_or_else(|| format!("Package '{}' not found", name).into())
}

// --- GitHub PKGBUILD helpers ---
// Fetch PKGBUILD from the GitHub aur mirror branch for package `pkg`
// (raw URL: https://raw.githubusercontent.com/archlinux/aur/<branch>/PKGBUILD)
fn fetch_pkgbuild_from_github(pkg: &str) -> Result<Option<String>, Box<dyn Error>> {
    let url = format!("{}/{}/PKGBUILD", GITHUB_AUR_MIRROR_RAW_BASE, pkg);
    let resp = get(&url)?;
    if !resp.status().is_success() {
        // Not found or HTTP error
        return Ok(None);
    }
    let body = resp.text()?;
    Ok(Some(body))
}

// Parse pkgver and pkgrel from PKGBUILD text.
// Returns combined version string like "1.2.3-4" (pkgver-pkgrel), or just "1.2.3" if pkgrel missing.
fn parse_pkgbuild_version(build: &str) -> Option<String> {
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

// --- helper to get installed AUR packages and their installed versions ---
// returns Vec<(name, version_string)>
fn get_installed_aur() -> Result<Vec<(String, String)>, Box<dyn Error>> {
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

// --- other helpers ---
fn fetch_github_packages() -> Result<Vec<String>, Box<dyn Error>> {
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

fn github_package_exists(pkg: &str, list: &[String]) -> bool {
    list.contains(&pkg.to_string())
}

// Return true for packages that are debug variants and should be ignored
fn is_debug_package(name: &str) -> bool {
    let s = name.to_lowercase();
    s.ends_with("-debug") ||
        s.ends_with("-dbg") ||
        s.ends_with("-dbgsym") ||
        s.ends_with("-debuginfo")
}

// --- Command implementations ---

fn cmd_search(term: &str, use_github: bool) -> Result<(), Box<dyn Error>> {
    if use_github {
        println!("searching github mirror for '{}'", term);
        let branches = fetch_github_packages()?;
        let mut matches: Vec<&String> = branches
            .iter()
            .filter(|b| b.contains(term))
            .collect();
        matches.sort();
        println!("\nFound {} packages (github mirror):", matches.len());
        for pkg in matches {
            println!("\n{}", pkg);
        }
        return Ok(());
    }

    let packages = fetch_search(term)?;
    println!("\nFound {} packages:", packages.len());
    for pkg in packages {
        println!("\n{} {}", pkg.name, pkg.version.as_deref().unwrap_or(""));
        if let Some(desc) = &pkg.description {
            println!("  {}", desc);
        }
        println!("  Popularity: {:.2}", pkg.popularity.unwrap_or(0.0));
    }
    Ok(())
}

fn cmd_install(pkgs: &[String], use_github: bool) -> Result<(), Box<dyn Error>> {
    let github_list = if use_github { Some(fetch_github_packages()?) } else { None };

    for pkg_name in pkgs {
        if is_debug_package(pkg_name) {
            // avoid cloning/building debug packages explicitly
            println!("Skipping debug package install request: {}", pkg_name);
            continue;
        }

        if use_github {
            if !github_package_exists(pkg_name, github_list.as_ref().unwrap()) {
                eprintln!("package '{}' not found on github mirror, skipping", pkg_name);
                continue;
            }

            println!("\nInstalling from github mirror: {}", pkg_name);
            if !prompt_yes("Proceed?") {
                println!("Skipping {}", pkg_name);
                continue;
            }

            let status = Shell::new("git")
                .arg("clone")
                .arg("--single-branch")
                .arg("--branch")
                .arg(pkg_name)
                .arg("https://github.com/archlinux/aur.git")
                .arg(pkg_name)
                .status()?;

            if !status.success() {
                eprintln!("git clone failed for {} (mirror).", pkg_name);
                continue;
            }

            let remove_deps = prompt_yes("Remove make dependencies after build?");
            let mut args = vec!["-si", "--noconfirm"];
            if remove_deps {
                args.push("--rmdeps");
            }

            let status = Shell::new("makepkg").args(&args).current_dir(pkg_name).status()?;
            let _ = fs::remove_dir_all(pkg_name);

            if status.success() {
                println!("Successfully installed {}", pkg_name);
            } else {
                eprintln!("Failed to install {} (build error).", pkg_name);
            }
        } else {
            let pkg = match fetch_info(pkg_name) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("failed to fetch info for {}: {}", pkg_name, e);
                    continue;
                }
            };

            println!("\nInstalling: {} {}", pkg.name, pkg.version.as_deref().unwrap_or(""));
            if !prompt_yes("Proceed?") {
                println!("Skipping {}", pkg.name);
                continue;
            }

            let repo_url = format!("https://aur.archlinux.org/{}.git", pkg.name);
            let status = Shell::new("git").arg("clone").arg(&repo_url).status()?;
            if !status.success() {
                eprintln!("git clone failed for {} (aur).", pkg.name);
                continue;
            }

            let remove_deps = prompt_yes("Remove make dependencies after build?");
            let mut args = vec!["-si", "--noconfirm"];
            if remove_deps {
                args.push("--rmdeps");
            }

            let status = Shell::new("makepkg").args(&args).current_dir(&pkg.name).status()?;
            let _ = fs::remove_dir_all(&pkg.name);

            if status.success() {
                println!("Successfully installed {}", pkg.name);
            } else {
                eprintln!("Failed to install {} (build error).", pkg.name);
            }
        }
    }
    Ok(())
}

// --- Update logic: compare installed version to PKGBUILD version (GitHub) or AUR RPC (normal)
fn cmd_update(use_github: bool) -> Result<(), Box<dyn Error>> {
    println!("Checking for updates...");

    let installed = get_installed_aur()?;
    if installed.is_empty() {
        println!("No AUR packages installed");
        return Ok(());
    }

    let mut to_update: Vec<String> = Vec::new();

    for (name, installed_ver) in installed {
        if is_debug_package(&name) {
            println!("Skipping debug package: {}", name);
            continue;
        }

        if use_github {
            // try to fetch PKGBUILD quickly via raw GitHub URL and parse pkgver/pkgrel
            match fetch_pkgbuild_from_github(&name) {
                Ok(Some(pkgb)) => {
                    if let Some(remote_ver) = parse_pkgbuild_version(&pkgb) {
                        if remote_ver != installed_ver {
                            to_update.push(name.clone());
                        }
                        continue;
                    } else {
                        // Could not parse PKGBUILD (dynamic pkgver). Fall back to AUR RPC if possible.
                        eprintln!("Could not parse PKGBUILD version for {}; falling back to AUR RPC", name);
                    }
                }
                Ok(None) => {
                    eprintln!("No PKGBUILD found for {} on GitHub mirror; falling back to AUR RPC", name);
                }
                Err(e) => {
                    eprintln!(
                        "Error fetching PKGBUILD for {}: {}; falling back to AUR RPC",
                        name,
                        e
                    );
                }
            }
            // fallback to RPC if github PKGBUILD missing or unparseable
            match fetch_info(&name) {
                Ok(pkg) => {
                    let rpc_ver = pkg.version.unwrap_or_default();
                    if rpc_ver != installed_ver {
                        to_update.push(name.clone());
                    }
                }
                Err(e) => {
                    eprintln!("Cannot fetch AUR RPC info for {}: {}; skipping", name, e);
                }
            }
        } else {
            // normal AUR RPC path
            match fetch_info(&name) {
                Ok(pkg) => {
                    let rpc_ver = pkg.version.unwrap_or_default();
                    if rpc_ver != installed_ver {
                        to_update.push(name.clone());
                    }
                }
                Err(e) => {
                    eprintln!("Cannot fetch AUR RPC info for {}: {}; skipping", name, e);
                }
            }
        }
    }

    if to_update.is_empty() {
        println!("All AUR packages are up-to-date");
        return Ok(());
    }

    println!("Updating {} package(s)...", to_update.len());
    cmd_install(&to_update, use_github)?;
    Ok(())
}

fn cmd_info(pkg_name: &str, use_github: bool) -> Result<(), Box<dyn Error>> {
    if use_github {
        match fetch_pkgbuild_from_github(pkg_name)? {
            Some(pkgb) => {
                if let Some(ver) = parse_pkgbuild_version(&pkgb) {
                    println!("\nPackage: {} (from github mirror)", pkg_name);
                    println!("Version (from PKGBUILD): {}", ver);
                    println!("Source: https://github.com/archlinux/aur (branch = pkg name)");
                    println!(
                        "Note: PKGBUILD parsing is naive; some PKGBUILDs compute version dynamically."
                    );
                    return Ok(());
                } else {
                    println!("PKGBUILD found but version could not be parsed (dynamic/complex).");
                }
            }
            None => {
                return Err(format!("package '{}' not found on github mirror", pkg_name).into());
            }
        }
    }
    let pkg = fetch_info(pkg_name)?;
    println!("\nPackage: {}", pkg.name);
    println!("Version: {}", pkg.version.as_deref().unwrap_or("Unknown"));
    println!("Maintainer: {}", pkg.maintainer.as_deref().unwrap_or("None"));
    println!("Popularity: {:.2}", pkg.popularity.unwrap_or(0.0));
    if !pkg.description.as_ref().map_or(true, |s| s.is_empty()) {
        println!("\nDescription:\n  {}", pkg.description.unwrap());
    }
    if !pkg.depends.is_empty() {
        println!("\nDependencies:");
        for dep in &pkg.depends {
            println!("  - {}", dep);
        }
    }
    if !pkg.make_depends.is_empty() {
        println!("\nBuild Dependencies:");
        for dep in &pkg.make_depends {
            println!("  - {}", dep);
        }
    }
    Ok(())
}

fn cmd_clean() -> Result<(), Box<dyn Error>> {
    println!("Cleaning build directories...");
    for entry in fs::read_dir(".")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().into_string().unwrap();
        let pkgbuild_path = format!("{}/PKGBUILD", dir_name);
        if fs::metadata(pkgbuild_path).is_ok() {
            fs::remove_dir_all(&dir_name)?;
            println!("Removed: {}", dir_name);
        }
    }
    Ok(())
}

fn cmd_uninstall(pkgs: &[String]) -> Result<(), Box<dyn Error>> {
    for pkg in pkgs {
        if !prompt_yes(&format!("Really uninstall {}?", pkg)) {
            println!("Skipping {}", pkg);
            continue;
        }
        let status = Shell::new("sudo").arg("pacman").arg("-Rns").arg(pkg).status()?;
        if status.success() {
            println!("Successfully removed {}", pkg);
        } else {
            eprintln!("Failed to remove {}", pkg);
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("raur")
        .version("1.2")
        .about("Simple AUR Helper")
        .arg(
            Arg::new("github")
                .long("github")
                .help("Use GitHub mirror instead of AUR RPC (global flag)")
                .global(true)
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("meow")
                .long("meow")
                .help("meow (necessary feature)")
                .global(true)
                .action(ArgAction::SetTrue)
        )
        .subcommand_required(false)
        .subcommand(
            Command::new("search")
                .about("Search AUR packages")
                .arg(Arg::new("query").required(true))
        )
        .subcommand(
            Command::new("install")
                .about("Install AUR packages")
                .arg(
                    Arg::new("packages")
                        .required(true)
                        .num_args(1..)
                )
                .alias("i")
        )
        .subcommand(Command::new("update").about("Update installed AUR packages").alias("u"))
        .subcommand(
            Command::new("info")
                .about("Show package information")
                .arg(Arg::new("package").required(true))
        )
        .subcommand(Command::new("clean").about("Clean build directories"))
        .subcommand(
            Command::new("uninstall")
                .about("Uninstall AUR packages")
                .arg(
                    Arg::new("packages")
                        .required(true)
                        .num_args(1..)
                )
                .alias("r")
        )
        .get_matches();

    if matches.get_flag("meow") {
        println!("meow (necessary feature)");
        return Ok(());
    }

    let use_github = matches.get_flag("github");

    if matches.subcommand().is_none() {
        eprintln!("error: 'raur' requires a subcommand but one was not provided");
        eprintln!("\nFor more information, try '--help'.");
        std::process::exit(1);
    }

    match matches.subcommand() {
        Some(("search", sub_m)) =>
            cmd_search(sub_m.get_one::<String>("query").unwrap(), use_github)?,
        Some(("install", sub_m)) => {
            let packages: Vec<String> = sub_m
                .get_many::<String>("packages")
                .unwrap()
                .cloned()
                .collect();
            cmd_install(&packages, use_github)?;
        }
        Some(("update", _)) => cmd_update(use_github)?,
        Some(("info", sub_m)) => cmd_info(sub_m.get_one::<String>("package").unwrap(), use_github)?,
        Some(("clean", _)) => cmd_clean()?,
        Some(("uninstall", sub_m)) => {
            let packages: Vec<String> = sub_m
                .get_many::<String>("packages")
                .unwrap()
                .cloned()
                .collect();
            cmd_uninstall(&packages)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
