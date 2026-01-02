mod aur_rpc;
mod cli;
mod fetching;
mod github;
mod helpers;

use aur_rpc::{ fetch_info, fetch_search };
use fetching::{ get_installed_aur, is_debug_package };
use github::{
    fetch_github_packages,
    fetch_pkgbuild_from_github,
    github_package_exists,
    parse_pkgbuild_version,
    PkgbuildState
};
use helpers::{ check_root, prompt_yes };
use std::error::Error;
use std::{ fs };
use std::process::Command as Shell;

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
fn cmd_update(use_github: bool, bypass: &bool) -> Result<(), Box<dyn Error>> {
    check_root(bypass);

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
                Ok(PkgbuildState::Result(pkgb)) => {
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
                Ok(PkgbuildState::NotFound) => {
                    return Err(format!("package '{}' not found on github mirror", name).into());
                }
                Ok(PkgbuildState::OtherError) => {
                    return Err(
                        format!("package '{}' returned non-200 return code when fetching", name).into()
                    );
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
            PkgbuildState::Result(pkgb) => {
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
            PkgbuildState::NotFound => {
                return Err(format!("package '{}' not found on github mirror", pkg_name).into());
            }
            PkgbuildState::OtherError => {
                return Err(
                    format!("package '{}' returned non-200 return code when fetching", pkg_name).into()
                );
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

fn cmd_uninstall(pkgs: &[String], bypass: &bool) -> Result<(), Box<dyn Error>> {
    check_root(&bypass);

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
    let matches = cli::build_cli().get_matches();

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

    let bypass = matches.get_flag("bypass-sudo");

    match matches.subcommand() {
        Some(("search", sub_m)) => {
            cmd_search(sub_m.get_one::<String>("query").unwrap(), use_github)?;
        }
        Some(("install", sub_m)) => {
            check_root(&bypass);

            let packages: Vec<String> = sub_m
                .get_many::<String>("packages")
                .unwrap()
                .cloned()
                .collect();
            cmd_install(&packages, use_github)?;
        }
        Some(("update", _)) => cmd_update(use_github, &bypass)?,
        Some(("info", sub_m)) => cmd_info(sub_m.get_one::<String>("package").unwrap(), use_github)?,
        Some(("clean", _)) => cmd_clean()?,
        Some(("uninstall", sub_m)) => {
            let packages: Vec<String> = sub_m
                .get_many::<String>("packages")
                .unwrap()
                .cloned()
                .collect();
            cmd_uninstall(&packages, &bypass)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
