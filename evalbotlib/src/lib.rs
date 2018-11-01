extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate toml;

use std::collections::HashMap;

pub mod util;

fn empty_string() -> String { "".to_owned() }

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Debug)]
pub struct EvalService {
    timeout: usize,
    languages: HashMap<String, Language>
}

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Debug)]
pub struct Language {
    timeout: Option<usize>,
    #[serde(skip)]
    #[serde(default = "empty_string")]
    name: String,
    code_before: Option<String>,
    code_after: Option<String>,
    #[serde(flatten)]
    backend: Option<Backend>
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum Backend {
    Exec {
        path: String,
        args: Vec<String>,
        timeout_prefix: Option<String>
    },
    Network {
        network_addr: String
    },
    UnixSocket {
        socket_addr: String
    }
}

impl EvalService {
    fn fixup(mut self) -> Self {
        for (name, mut lang) in self.languages.iter_mut() {
            lang.name = name.clone();
        }
        self
    }

    pub fn from_toml_file(path: &str) -> Result<Self, String> {
        util::decode(path).map(EvalService::fixup)
    }

    pub fn from_toml(toml: &str) -> Result<Self, String> {
        toml::from_str(toml).map(EvalService::fixup).map_err(|x| format!("could not parse TOML: {:?}", x))
    }
}

fn wrap_code(raw: &str, cfg: &Language) -> String {
    let mut code = String::with_capacity(raw.len());

    if let Some(ref prefix) = cfg.code_before {
        code.push_str(prefix);
    }

    code.push_str(raw);

    if let Some(ref postfix) = cfg.code_after {
        code.push_str(postfix);
    }

    code
}

#[cfg(test)]
mod test {
    use toml;

    #[test]
    fn test_decode() {
        let toml = r#"
timeout = 20

[languages.rs]
path = "rustc"
args = ["-O"]

[languages.'rs!']
timeout = 0
path = "rustc"
args = ["-O"]
"#;
        println!("{:#?}", super::EvalService::from_toml(toml).unwrap());
    }
}
