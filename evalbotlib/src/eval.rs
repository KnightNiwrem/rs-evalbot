use std::process::{Command, Stdio};
use std::ffi::{OsStr, OsString};

use futures::Future;
use futures::future::IntoFuture;
use tokio_process::{CommandExt, Child};
use tokio_io::io::write_all;
use std::io::Write;

pub fn exec<'a, I, S, T>(
    path: &str,
    args: I,
    timeout: Option<usize>,
    timeout_prefix: Option<&str>,
    code_before: T,
    code_after: T,
    code: T) -> impl Future<Item = String, Error = String> + 'a
        where
            I: IntoIterator<Item = S>,
            S: AsRef<str> + PartialEq + 'a,
            T: AsRef<[u8]> + 'a {
    let timeout_arg = timeout
        .map(|t| format!("{}{}", timeout_prefix.unwrap_or(""), t));
    {
        let mut cmd = Command::new(path);
        for arg in args {
            if arg.as_ref() == "{TIMEOUT}" && timeout_arg.is_some() {
                cmd.arg(timeout_arg.as_ref().expect("is_some was true"));
            } else {
                cmd.arg(arg.as_ref());
            }
        }
        cmd
    }
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn_async()
        .map_err(|e| format!("failed to exec: {}", e))
        .into_future()
        .and_then(|mut child| {
            child.stdin().take().ok_or_else(|| "stdin missing".to_owned()).into_future()
                .and_then(|stdin|
                    write_all(stdin, code_before)
                        .and_then(|(stdin, _)|
                            write_all(stdin, code)
                        )
                        .and_then(|(stdin, _)|
                            write_all(stdin, code_after)
                        )
                        .map_err(|e| format!("failed to write to stdin: {}", e))
                )
                .and_then(|_| child.wait_with_output()
                    .map_err(|e| format!("failed to wait for process: {}", e)))
        })
        .map_err(|e| format!("unknown error in exec: {}", e))
        .map(|o| format!("{}{}",
            String::from_utf8_lossy(&o.stderr),
            String::from_utf8_lossy(&o.stdout),
        ))
}
