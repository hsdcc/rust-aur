use std::io::{self, Write};

use raur_lib::{YES_OPTIONS, NO_OPTIONS};
use nix::unistd::Uid;
use std::process::exit;

// simple yes/no prompt
pub fn prompt_yes(question: &str) -> bool {
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

pub fn check_root(bypass: &bool) {
    // Check is user is root         Check if a bypass flag was provided
    if Uid::effective().is_root() && !*bypass {
        println!(
            "Running this program with root privileges is not supported. Use --bypass-sudo to use bypass this."
        );
        exit(1);
    }
}