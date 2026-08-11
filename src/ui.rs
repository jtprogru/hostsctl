//! Минимальный вывод в терминал: цвета только когда есть tty и нет NO_COLOR.

use std::sync::OnceLock;

fn colors() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }
        if std::env::var("TERM").is_ok_and(|t| t == "dumb") {
            return false;
        }
        // SAFETY: isatty на валидном fd, побочных эффектов нет.
        unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
    })
}

fn paint(code: &str, s: &str) -> String {
    if colors() { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
}

pub fn bold(s: &str) -> String {
    paint("1", s)
}
pub fn dim(s: &str) -> String {
    paint("2", s)
}
pub fn red(s: &str) -> String {
    paint("31", s)
}
pub fn green(s: &str) -> String {
    paint("32", s)
}
pub fn yellow(s: &str) -> String {
    paint("33", s)
}
pub fn cyan(s: &str) -> String {
    paint("36", s)
}

/// Схлопывает одинаковые сообщения, сохраняя порядок: шесть одинаковых
/// предупреждений про один и тот же список читать невозможно.
pub fn dedup_counted(items: &[String]) -> Vec<String> {
    let mut order: Vec<String> = vec![];
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for i in items {
        if counts.insert(i.as_str(), 1).is_none() {
            order.push(i.clone());
        } else {
            *counts.get_mut(i.as_str()).expect("inserted above") += 1;
        }
    }
    order
        .into_iter()
        .map(|msg| match counts.get(msg.as_str()) {
            Some(&n) if n > 1 => format!("{msg} (×{n})"),
            _ => msg,
        })
        .collect()
}

pub fn warn(msg: &str) {
    eprintln!("{} {msg}", yellow("warn:"));
}
pub fn ok(msg: &str) {
    println!("{} {msg}", green("ok:"));
}
pub fn info(msg: &str) {
    println!("{} {msg}", cyan("::"));
}

/// Спрашивает подтверждение. Если stdin не tty — считаем ответ отрицательным.
pub fn confirm(question: &str) -> bool {
    // SAFETY: isatty на валидном fd.
    if unsafe { libc::isatty(libc::STDIN_FILENO) } != 1 {
        eprintln!("{question} — stdin is not interactive, pass --yes");
        return false;
    }
    eprint!("{question} [y/N] ");
    use std::io::Write;
    let _ = std::io::stderr().flush();
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        return false;
    }
    matches!(buf.trim().to_lowercase().as_str(), "y" | "yes" | "д" | "да")
}
