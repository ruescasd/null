//! Every option the game reads, gathered from its source (`opt("name")`,
//! `num("name", ...)`, and the console's lists of `("name", ...)`), so the
//! command line can say when it is given one nothing reads (the user: a
//! mistyped `--set` silently did nothing). Written to `options.rs` in
//! `OUT_DIR`, as `KNOWN`.

use std::{fs, path::Path};

fn main() {
    let mut names = Vec::new();
    gather(Path::new("src"), &mut names);
    names.sort();
    names.dedup();
    let list: String = names.iter().map(|n| format!("    {n:?},\n")).collect();
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("options.rs");
    fs::write(out, format!("/// Every option name the game reads (see `build.rs`).\npub const KNOWN: &[&str] = &[\n{list}];\n")).expect("writing options.rs");
    println!("cargo:rerun-if-changed=src");
}

fn gather(dir: &Path, names: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("reading src").flatten() {
        let path = entry.path();
        if path.is_dir() {
            gather(&path, names);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).expect("reading a source file");
            for key in ["opt(\"", "num(\""] {
                names.extend(after(&text, key));
            }
            // (The console's own lists: its switches and numbers, read by
            // a name held in them.)
            if path.file_name().is_some_and(|f| f == "console.rs") {
                names.extend(after(&text, "(\""));
            }
        }
    }
}

/// The names (letters, digits, `_`) quoted straight after each `key`.
fn after(text: &str, key: &str) -> Vec<String> {
    text.match_indices(key)
        .filter_map(|(i, _)| {
            let rest = &text[i + key.len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            (!name.is_empty() && rest[name.len()..].starts_with('"')).then_some(name)
        })
        .collect()
}
