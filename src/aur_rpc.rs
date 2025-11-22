use raur_lib::{AurPkg, RpcResponse, AUR_RPC};
use reqwest::blocking::get;
use std::error::Error;

pub fn fetch_search(term: &str) -> Result<Vec<AurPkg>, Box<dyn Error>> {
    let url = format!("{}type=search&arg={}", AUR_RPC, term);
    let resp: RpcResponse = get(&url)?.json()?;
    let mut packages = resp.results;
    packages.sort_by(|a, b| {
        b.popularity.unwrap_or(0.0).partial_cmp(&a.popularity.unwrap_or(0.0)).unwrap()
    });
    Ok(packages)
}

pub fn fetch_info(name: &str) -> Result<AurPkg, Box<dyn Error>> {
    let url = format!("{}type=info&arg={}", AUR_RPC, name);
    let resp: RpcResponse = get(&url)?.json()?;
    resp.results
        .into_iter()
        .next()
        .ok_or_else(|| format!("Package '{}' not found", name).into())
}