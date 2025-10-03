# raur

A simple AUR helper written in rust.

## Usage

```bash
raur [OPTIONS] <COMMAND>
```

As of now, 7 commands are available:

| Command     | Alias | Description                            |
| ----------- | :---: | -------------------------------------- |
| `search`    |       | Searches for AUR package(s)            |
| `install`   | `i`   | Installs an AUR package                |
| `update`    | `u`   | Updates an installed AUR package       |
| `info`      |       | Shows information about an AUR package |
| `clean`     |       | Cleans build directories               |
| `uninstall` | `r`   | Uninstalls an installed AUR package    |
| `help`      |       | Provides help on how to use this tool  |

3 flags are also available:

| Flag            | Description                                         |
| --------------- | --------------------------------------------------- |
| `--github`      | Use the GitHub AUR mirror for package installation. |
| `--meow`        | Meows at you (requires paid subscription /j)        |
| `--bypass-sudo` | Allows the program to run with sudo privileges.     |

## Building

To build this project, run:

```bash
cargo install path ./the/path/to/project's/root
```

Or, using the github aur mirror (recommended, fetches all dependencies automatically):

```bash
 git clone --single-branch --branch raur-helper-git https://github.com/archlinux/aur.git raur-helper-git
 cd raur-helper-git
 makepkg -si
```
