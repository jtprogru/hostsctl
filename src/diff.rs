//! Unified diff между текущим и будущим содержимым файла.

use crate::ui;
use similar::{ChangeTag, TextDiff};

pub fn render(old: &str, new: &str, label: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();
    out.push_str(&ui::bold(&format!("--- {label}\n+++ {label} (after apply)\n")));

    for (i, group) in diff.grouped_ops(3).iter().enumerate() {
        if i > 0 {
            out.push_str(&ui::dim("...\n"));
        }
        for op in group {
            for change in diff.iter_changes(op) {
                let (sign, painter): (&str, fn(&str) -> String) = match change.tag() {
                    ChangeTag::Delete => ("-", ui::red),
                    ChangeTag::Insert => ("+", ui::green),
                    ChangeTag::Equal => (" ", ui::dim),
                };
                let text = change.to_string_lossy();
                out.push_str(&painter(&format!("{sign}{}", text.trim_end())));
                out.push('\n');
            }
        }
    }
    out
}

/// Компактная сводка: сколько строк добавлено/удалено.
pub fn summary(old: &str, new: &str) -> (usize, usize) {
    let diff = TextDiff::from_lines(old, new);
    let mut add = 0;
    let mut del = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => add += 1,
            ChangeTag::Delete => del += 1,
            ChangeTag::Equal => {}
        }
    }
    (add, del)
}
