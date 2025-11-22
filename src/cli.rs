use clap::{Arg, ArgAction, Command};

pub fn build_cli() -> Command {
    Command::new("raur")
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
        .arg(
            Arg::new("bypass-sudo")
                .long("bypass-sudo")
                .help("Bypass root verification (not recommended)")
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
}