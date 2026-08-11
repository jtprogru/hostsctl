//! Коды выхода и их привязка к ошибкам.
//!
//! Скрипт, который зовёт `hostsctl apply` из automation, должен уметь отличить
//! «конфиг сломан» от «не хватило прав»: первое чинит человек, второе — sudo.
//! Поэтому ошибка в нужных местах помечается кодом, а `main` его выставляет.

use std::fmt;

/// Everything went fine.
pub const OK: i32 = 0;
/// Something went wrong and hostsctl has no more specific code for it.
pub const FAILURE: i32 = 1;
/// The command line itself was wrong; emitted by clap, never by hostsctl.
pub const USAGE: i32 = 2;
/// The config is missing, unreadable, invalid, or holds errors that check reports.
pub const CONFIG: i32 = 3;
/// The target file or the backup directory is not writable — retry under sudo.
pub const PERMISSION: i32 = 4;
/// Reading or writing a file failed for a reason other than permissions.
pub const IO: i32 = 5;
/// A remote blocklist could not be downloaded.
pub const NETWORK: i32 = 6;

const TABLE: &[(i32, &str)] = &[
    (OK, "Everything went fine."),
    (FAILURE, "Something went wrong and hostsctl has no more specific code for it."),
    (USAGE, "The command line itself was wrong; emitted by clap, never by hostsctl."),
    (CONFIG, "The config is missing, unreadable, invalid, or holds errors that check reports."),
    (PERMISSION, "The target file or the backup directory is not writable — retry under sudo."),
    (IO, "Reading or writing a file failed for a reason other than permissions."),
    (NETWORK, "A remote blocklist could not be downloaded."),
];

/// Ошибка, которая знает, с каким кодом должен завершиться процесс.
///
/// Display повторяет вложенную ошибку целиком (`{:#}`), поэтому обёртка не
/// добавляет в сообщение ни одного лишнего символа.
#[derive(Debug)]
pub struct Coded {
    pub code: i32,
    source: anyhow::Error,
}

impl fmt::Display for Coded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#}", self.source)
    }
}

impl std::error::Error for Coded {}

/// Помечает ошибку кодом выхода.
pub fn coded(code: i32, err: impl Into<anyhow::Error>) -> anyhow::Error {
    anyhow::Error::new(Coded { code, source: err.into() })
}

/// Тот же `?`, но с кодом: `some_call().or_code(exit::CONFIG)?`.
pub trait OrCode<T> {
    fn or_code(self, code: i32) -> anyhow::Result<T>;
}

impl<T, E: Into<anyhow::Error>> OrCode<T> for Result<T, E> {
    fn or_code(self, code: i32) -> anyhow::Result<T> {
        self.map_err(|e| coded(code, e))
    }
}

/// Код, с которым надо завершиться из-за этой ошибки.
pub fn code_of(err: &anyhow::Error) -> i32 {
    err.chain().find_map(|e| e.downcast_ref::<Coded>().map(|c| c.code)).unwrap_or(FAILURE)
}

/// Таблица для `hostsctl docs exit-codes`.
pub fn table_md() -> String {
    let mut out = String::from("| Code | Meaning |\n| --- | --- |\n");
    for (code, meaning) in TABLE {
        out.push_str(&format!("| `{code}` | {meaning} |\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn plain_errors_are_generic_failures() {
        assert_eq!(code_of(&anyhow!("boom")), FAILURE);
    }

    #[test]
    fn coded_errors_keep_their_code_and_message() {
        let e = coded(PERMISSION, anyhow!("no write access to /etc/hosts"));
        assert_eq!(code_of(&e), PERMISSION);
        assert_eq!(format!("{e:#}"), "no write access to /etc/hosts");
    }

    #[test]
    fn a_code_survives_further_context() {
        use anyhow::Context;
        let e = Err::<(), _>(coded(CONFIG, anyhow!("bad yaml")))
            .context("cannot read the config")
            .unwrap_err();
        assert_eq!(code_of(&e), CONFIG);
        assert_eq!(format!("{e:#}"), "cannot read the config: bad yaml");
    }
}
