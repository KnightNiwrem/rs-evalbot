use std::io::prelude::*;
use std::fs::File;

use serde::{Serialize, de::DeserializeOwned};

pub fn encode<T: Serialize>(obj: &T, name: &str) -> Result<(), String> {
    let toml = toml::to_string(obj).map_err(|e| format!("toml encode failed: {}", e))?;
    let mut f = File::create(name).map_err(|x| format!("could not open {}: {}", name, x))?;
    f.write_all(toml.as_bytes()).map(|_| ()).map_err(|x| format!("could not write to {}: {}", name, x))
}

pub fn decode<T: DeserializeOwned>(name: &str) -> Result<T, String> {
    let mut f = File::open(name).map_err(|x| format!("could not open {}: {}", name, x))?;
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|x| format!("could not read {}: {}", name, x))?;

    toml::from_str(&s).map_err(|x| format!("could not parse {}: {:?}", name, x))
}
