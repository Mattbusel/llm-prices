//! Walks a directory for files to scan.

use std::fs;
use std::path::Path;

use regex::Regex;

const SKIP_DIRS: [&str; 14] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".tox",
    "coverage",
    ".cache",
    "site-packages",
];

/// A glob (`**`, `*`, `?`, `{a,b}`) as a regex over forward-slash relative paths.
pub fn glob_to_regex(glob: &str) -> Regex {
    let mut g = glob.trim().trim_start_matches("./").to_string();
    if g.ends_with('/') {
        g.push_str("**");
    }
    let chars: Vec<char> = g.chars().collect();
    let mut re = String::from("^");
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                if chars.get(i + 1) == Some(&'*') {
                    if chars.get(i + 2) == Some(&'/') {
                        re.push_str("(?:.*/)?");
                        i += 2;
                    } else {
                        re.push_str(".*");
                        i += 1;
                    }
                } else {
                    re.push_str("[^/]*");
                }
            }
            '?' => re.push_str("[^/]"),
            '{' => {
                let end = chars[i..]
                    .iter()
                    .position(|&x| x == '}')
                    .map(|p| p + i)
                    .unwrap_or(chars.len());
                let inner: String = chars[i + 1..end].iter().collect();
                let alts: Vec<String> = inner
                    .split(',')
                    .map(|s| regex::escape(s).replace("\\*", "[^/]*"))
                    .collect();
                re.push_str(&format!("(?:{})", alts.join("|")));
                i = end;
            }
            c => re.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
    re.push('$');
    Regex::new(&re).unwrap_or_else(|_| Regex::new("$^").expect("valid regex"))
}

/// Splits a comma or newline separated list.
pub fn split_list(s: &str) -> Vec<String> {
    s.split([',', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

/// Relative paths under `root` matching an include glob and no exclude glob, sorted.
pub fn list_files(
    root: &Path,
    include: &[String],
    exclude: &[String],
    max_bytes: u64,
) -> Vec<String> {
    let inc: Vec<Regex> = if include.is_empty() {
        vec![glob_to_regex("**")]
    } else {
        include.iter().map(|g| glob_to_regex(g)).collect()
    };
    let exc: Vec<Regex> = exclude.iter().map(|g| glob_to_regex(g)).collect();
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel_dir)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let rel = if rel_dir.is_empty() {
                name.clone()
            } else {
                format!("{rel_dir}/{name}")
            };
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                let with_slash = format!("{rel}/");
                if exc
                    .iter()
                    .any(|r| r.is_match(&rel) || r.is_match(&with_slash))
                {
                    continue;
                }
                stack.push((e.path(), rel));
            } else if ft.is_file() {
                if !inc.iter().any(|r| r.is_match(&rel)) || exc.iter().any(|r| r.is_match(&rel)) {
                    continue;
                }
                if e.metadata().map(|m| m.len() > max_bytes).unwrap_or(true) {
                    continue;
                }
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}
