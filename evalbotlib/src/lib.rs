extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate toml;
extern crate tokio;
extern crate tokio_process;
extern crate futures;

use std::collections::HashMap;
use futures::Future;
use futures::future::Either;
use std::path::Path;
use std::fmt::Display;

pub mod util;
mod eval;

fn empty_string() -> String { "".to_owned() }

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Debug)]
pub struct EvalService {
    timeout: usize,
    languages: HashMap<String, Language>
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Language {
    timeout: Option<usize>,
    #[serde(skip)]
    #[serde(default = "empty_string")]
    name: String,
    code_before: Option<String>,
    code_after: Option<String>,
    #[serde(flatten)]
    backend: Backend
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
        let timeout = self.timeout;
        for (name, mut lang) in self.languages.iter_mut() {
            lang.name = name.clone();
            lang.timeout = lang.timeout.or_else(|| Some(timeout));
        }
        self
    }

    pub fn from_toml_file<P>(path: P) -> impl Future<Item = Self, Error = String>
        where P: AsRef<Path> + Send + Display + 'static{
        util::decode(path).map(EvalService::fixup)
    }

    pub fn from_toml(toml: &str) -> Result<Self, String> {
        toml::from_str(toml).map(EvalService::fixup).map_err(|x| format!("could not parse TOML: {:?}", x))
    }

    pub fn get(&self, lang: &str) -> Option<&Language> {
        self.languages.get(lang)
    }
}

static EMPTY_U8: [u8; 0] = [];

impl Language {
    pub fn eval<'a>(&'a self, code: &'a str, timeout: Option<usize>) -> impl Future<Item = String, Error = String> + 'a {
        match self.backend {
            Backend::Exec { ref path, ref args, ref timeout_prefix } =>
                Either::A(eval::exec(&path, args, timeout.or(self.timeout),
                    timeout_prefix.as_ref().map(String::as_str),
                    self.code_before.as_ref().map(String::as_bytes).unwrap_or(&EMPTY_U8),
                    self.code_after.as_ref().map(String::as_bytes).unwrap_or(&EMPTY_U8),
                    code.as_bytes())),
            _ => Either::B(futures::finished("Unimplemented".to_owned()))
        }
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
