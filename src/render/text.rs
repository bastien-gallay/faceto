//! Pure text helpers: hump-splitting, wrapping, XML escaping, label/detail split.

pub(crate) fn is_upper(c: char) -> bool {
    c.is_ascii_uppercase()
}
pub(crate) fn is_lower_or_digit(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

/// Break a long CamelCase / Pascal token before a capital that follows a lower/digit, and
/// before the last capital of an acronym run — no space inserted.
pub(crate) fn hump_split(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    let mut cuts = vec![0usize];
    for i in 1..n {
        let prev = chars[i - 1];
        let cur = chars[i];
        let cond1 = is_lower_or_digit(prev) && is_upper(cur);
        let cond2 =
            is_upper(prev) && is_upper(cur) && i + 1 < n && chars[i + 1].is_ascii_lowercase();
        if cond1 || cond2 {
            cuts.push(i);
        }
    }
    cuts.push(n);
    cuts.windows(2)
        .map(|w| chars[w[0]..w[1]].iter().collect())
        .collect()
}

/// Break one over-long token into wrap-able pieces: CamelCase humps first, then hard char-split.
pub(crate) fn atoms(word: &str, width: usize) -> Vec<String> {
    let pieces = if word.chars().count() > width {
        hump_split(word)
    } else {
        vec![word.to_string()]
    };
    let mut out = Vec::new();
    for p in pieces {
        let mut chars: Vec<char> = p.chars().collect();
        while chars.len() > width {
            out.push(chars[..width].iter().collect());
            chars = chars[width..].to_vec();
        }
        out.push(chars.iter().collect());
    }
    out
}

/// CamelCase-aware greedy wrap. Pieces of one broken token rejoin with no space (`glued`).
pub(crate) fn wrap(label: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut toks: Vec<(String, bool)> = Vec::new();
    for word in label.split_whitespace() {
        for (j, piece) in atoms(word, width).into_iter().enumerate() {
            toks.push((piece, j > 0));
        }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (text, glued) in toks {
        let sep = if glued || cur.is_empty() { "" } else { " " };
        if !cur.is_empty()
            && cur.chars().count() + sep.chars().count() + text.chars().count() > width
        {
            lines.push(cur);
            cur = text;
        } else {
            cur = format!("{}{}{}", cur, sep, text);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last().unwrap().clone();
        let trimmed: String = last
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            .trim_end()
            .to_string();
        *lines.last_mut().unwrap() = format!("{}\u{2026}", trimmed); // …
    }
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// A sticky reads as a hero line + optional smaller detail. An explicit `detail` wins;
/// otherwise a trailing parenthetical becomes the detail.
pub(crate) fn split_label(label: &str, detail: Option<&str>) -> (String, String) {
    if let Some(d) = detail {
        if !d.is_empty() {
            return (label.trim().to_string(), d.trim().to_string());
        }
    }
    if let Some(i) = label.find('(') {
        let rstripped = label.trim_end();
        if i > 0 && rstripped.ends_with(')') {
            let close = rstripped.rfind(')').unwrap();
            return (
                label[..i].trim().to_string(),
                label[i + 1..close].trim().to_string(),
            );
        }
    }
    (label.trim().to_string(), String::new())
}

pub(crate) fn opt_col(c: Option<i64>) -> String {
    c.map(|v| v.to_string()).unwrap_or_else(|| "None".into())
}
