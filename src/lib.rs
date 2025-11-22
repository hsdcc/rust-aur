use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct RpcResponse {
    pub results: Vec<AurPkg>,
}

#[derive(Deserialize, Clone)]
pub struct AurPkg {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: Option<String>,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "Popularity")]
    pub popularity: Option<f32>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
    #[serde(rename = "Depends")]
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(rename = "MakeDepends")]
    #[serde(default)]
    pub make_depends: Vec<String>,
}

pub const AUR_RPC: &str = "https://aur.archlinux.org/rpc/?v=5&";
pub const GITHUB_AUR_MIRROR_RAW_BASE: &str = "https://raw.githubusercontent.com/archlinux/aur";

pub const YES_OPTIONS: [&'static str; 4] = ["y", "yes", "true", "yeah"];
pub const NO_OPTIONS: [&'static str; 4] = ["n", "not", "no", "nope"];