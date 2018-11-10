extern crate evalbotlib as backend;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate log;
extern crate toml;
extern crate futures;
extern crate tokio;
extern crate telebot;
extern crate env_logger;

use backend::{Backend, EvalService, Language, util};

use std::fmt::{Debug, Display};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use futures::{Future, Stream, IntoFuture};
use telebot::RcBot;
use telebot::functions::*;
use telebot::objects::*;
use futures::future::Either;

macro_rules! ignore_req {
    () => {
        {
            d!(println!("ignore_req!() @ {}:{}", file!(), line!()));
            return Ok("".to_owned());
        }
    }
}

#[cfg(feature = "debugprint")]
macro_rules! d { ($x:expr) => { $x } }
#[cfg(not(feature = "debugprint"))]
macro_rules! d { ($x:expr) => {} }

static WHITELIST_FILENAME: &'static str = "tgwhitelist.toml";

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct TgCfg {
    owners: HashSet<String>,
    msg_owner_id: Option<i64>,
    bot_id: String,
    lang_subst: HashMap<String, String>
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct TgWhitelist {
    priv_enabled: bool,
    group_enabled: bool,
    allowed: HashSet<i64>,
    blocked: HashSet<i64>
}

impl TgWhitelist {
    fn priv_ok(&self, id: i64) -> bool {
        (!self.priv_enabled || self.allowed.contains(&id)) && !self.blocked.contains(&id)
    }

    fn group_ok(&self, id: i64) -> bool {
        (!self.group_enabled || self.allowed.contains(&id)) && !self.blocked.contains(&id)
    }

    fn allow(&mut self, id: i64) {
        self.allowed.insert(id);
    }

    fn unallow(&mut self, id: i64) {
        self.allowed.remove(&id);
    }

    fn block(&mut self, id: i64) {
        self.blocked.insert(id);
    }

    fn unblock(&mut self, id: i64) {
        self.blocked.remove(&id);
    }

    fn save(&self, path: &'static str) -> impl Future<Item = (), Error = ()> + '_ {
        util::encode(&self, path)
            .map(|_| ())
            .map_err(|e| {
                warn!("failed to save whitelist: {}", e);
            })
    }
}

struct TgSvc {
    config: TgCfg,
    whitelist: RwLock<TgWhitelist>,
    service: EvalService,
}

fn handle_eval(lang: Arc<Language>, tgbot: RcBot, msg: Message)
    -> impl Future<Item = (), Error = ()> {
    let code = msg.text.unwrap_or("".to_owned());
    let chat_id = msg.chat.id;
    let msg_id = msg.message_id;
    lang.eval(code, None)
        .then(move |e| tgbot.message(chat_id, e.unwrap_or_else(|x| x))
            .reply_to_message_id(msg_id)
            .send())
        .map(|_| ())
        .map_err(|e| error!("error sending message: {}", e))
}

impl TgSvc {
    fn run() -> impl Future<Item = (), Error = ()> {
        let cfgf = util::decode::<TgCfg, _>("evalbot.tg.toml")
            .map(|cfg| {
                debug!("Loaded config: {:?}", cfg);
                cfg
            })
            .map_err(|e| {
                error!("failed to read evalbot.tg.toml: {}", e);
            });
        let wlf = util::decode::<TgWhitelist, _>(WHITELIST_FILENAME).or_else(|e| {
            warn!("failed to read whitelist: {}; using empty whitelist", e);
            Ok(TgWhitelist {
                priv_enabled: false,
                group_enabled: false,
                allowed: HashSet::new(),
                blocked: HashSet::new(),
            }).into_future()
        });
        cfgf.join(wlf).join(EvalService::from_toml_file("evalbot.toml")
            .map_err(|e| {
                error!("failed to read evalbot.toml: {}", e);
            }))
            .map(|((cfg, wl), es)| TgSvc {
                config: cfg,
                whitelist: RwLock::new(wl),
                service: es
            })
            .and_then(TgSvc::handle)
    }

    fn handle(self) -> impl Future<Item = (), Error = ()> {
        let bot = RcBot::new(&self.config.bot_id).expect("Failed to initialise Telegram bot");
        bot.resolve_name();
        for (name, lang) in self.service.langs() {
            let lang = lang.clone();
            bot.register(bot.new_cmd(name)
                .map_err(|e| error!("error in command processing: {}", e))
                .map(move |(x, y)| (lang.clone(), x, y))
                .and_then(|(lang, tgbot, msg)| handle_eval(lang, tgbot, msg)));
        }
        bot.get_stream()
            .for_each(|_| Ok(()))
            .map_err(|e| error!("{}", e))
            .into_future()
    }
}

fn main() {
    env_logger::init();
    tokio::run(TgSvc::run());
}
