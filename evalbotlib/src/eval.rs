use std::process::{Command, Stdio};
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use tokio::prelude::*;
use tokio::prelude::future::Either;
use tokio::timer::timeout;
use tokio_process::CommandExt;
use tokio::{io::{flush, read_exact, write_all}, net::unix::UnixStream};
use bytes::{BytesMut, Buf, BufMut};

use crate::{ExecBackend, UnixSocketBackend};

pub fn exec<'a, T>(
    lang: Arc<ExecBackend>,
    timeout: Option<usize>,
    code: T) -> impl Future<Item = String, Error = String> + 'a
        where T: AsRef<[u8]> + 'a {
    let timeout_arg = timeout
        .map(|t| format!("{}{}", lang.timeout_prefix.as_ref().map(String::as_str).unwrap_or(""), t));
    let timeout_arg_ref = timeout_arg.as_ref().map(String::as_str);
    if let Some(path) = lang.cmdline.iter().nth(0) {
        let mut cmd = Command::new(path);
        cmd.args(lang.cmdline.iter()
            .skip(1)
            .filter_map(|a| if a == "{TIMEOUT}" {
                timeout_arg_ref
            } else {
                Some(a.as_ref())
            }))
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        debug!("spawning {:?}", cmd);
        Either::A(cmd.spawn_async()
            .map_err(|e| format!("failed to exec: {}", e))
            .into_future()
            .and_then(|mut child| {
                child.stdin().take().ok_or_else(|| "stdin missing".to_owned()).into_future()
                    .and_then(|stdin|
                        write_all(stdin, code)
                            .map_err(|e| format!("failed to write to stdin: {}", e))
                    )
                    .and_then(|_| child.wait_with_output()
                        .map_err(|e| format!("failed to wait for process: {}", e)))
            })
            .map_err(|e| format!("unknown error in exec: {}", e))
            .map(|o| {
                let mut r = format!("{}{}",
                    String::from_utf8_lossy(&o.stderr),
                    String::from_utf8_lossy(&o.stdout),
                );
                if !o.status.success() {
                    if !r.ends_with('\n') {
                        r.push_str("\n");
                    }
                    if let Some(code) = o.status.code() {
                        r.push_str(&format!("exited with status {}\n", code));
                    } else {
                        r.push_str("exited with unknown failure\n");
                    }
                }
                r
            }))
    } else {
        Either::B(Err("empty cmdline".to_owned()).into_future())
    }
}

macro_rules! persistent {
    ($lang:expr, $connfut:expr, $timeout:expr, $buf:expr) => ({
        let buf = $buf;
        let fut = $connfut
            .map_err(|e| format!("error connecting: {}", e))
            .and_then(move |s| write_all(s, buf)
                .map_err(|e| format!("error writing: {}", e)))
            .and_then(|(s, _)| flush(s)
                .map_err(|e| format!("error flushing: {}", e)))
            .and_then(|s| read_exact(s, [0u8; 4])
                .map_err(|e| format!("error reading result length: {}", e)));
        if let Some(timeout) = $timeout {
            Either::A(fut.timeout(Duration::from_secs(timeout as u64)))
        } else {
            Either::B(fut.map_err(|e| timeout::Error::inner(e)))
        }.then(move |r| match r {
            Ok((s, lenb)) => Either::A(read_exact(s, {
                let outlen = Cursor::new(lenb).get_u32_le() as usize;
                let mut buf = BytesMut::with_capacity(outlen);
                buf.resize(outlen, 0);
                buf
            }).map_err(|e| format!("error reading result: {}", e))
                .map(|(_, ref outb)| String::from_utf8_lossy(outb).into_owned())),
            Err(e) => Either::B(if e.is_elapsed() {
                do_persistent_timeout(&$lang.timeout_cmdline);
                Ok("time limit exceeded".to_owned()).into_future()
            } else {
                Err(format!("error from timeout: {}", e)).into_future()
            })
        })
    });
}

pub fn unix<'a, T, U>(
    lang: Arc<UnixSocketBackend>,
    timeout: Option<usize>,
    context: Option<U>,
    code: T) -> impl Future<Item = String, Error = String> + 'a
        where
            T: AsRef<[u8]>,
            U: AsRef<[u8]> {
    persistent!(lang,
        UnixStream::connect(&lang.socket_addr),
        timeout,
        make_persistent_input(timeout, context, code))
}

fn do_persistent_timeout(cmdline: &Option<Vec<String>>) {
    if let Some(cmdline) = cmdline.as_ref() {
        if let Some(path) = cmdline.iter().nth(0) {
            debug!("timeout kill: launching {:?}", cmdline);
            tokio::spawn(Command::new(path)
                .args(cmdline.iter().skip(1))
                .spawn_async()
                .map_err(|e| error!("failed to exec for timeout kill: {}", e))
                .into_future()
                .and_then(|c| c
                    .map_err(|e| error!("failed to exec for timeout kill: {}", e)))
                    .map(|_| ()));
        }
    }
}

fn make_persistent_input<T, U>(timeout: Option<usize>, context: Option<T>, code: U) -> BytesMut
    where
        T: AsRef<[u8]>,
        U: AsRef<[u8]> {
    let timeout = timeout.unwrap_or(0usize) as u32;
    let contextb = context.as_ref().map(|x| x.as_ref()).unwrap_or(&super::EMPTY_U8);
    let codeb = code.as_ref();
    let contextblen = contextb.len() as u32;
    let codeblen = codeb.len() as u32;

    let mut buf = BytesMut::with_capacity(12usize + contextblen as usize + codeblen as usize);
    buf.put_u32_le(timeout*1000);
    buf.put_u32_le(contextblen);
    buf.put_u32_le(codeblen);
    buf.put(&contextb[..contextblen as usize]);
    buf.put(&codeb[..codeblen as usize]);
    buf
}
