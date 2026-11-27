//! The flags each command takes. One a command doesn't know is a warning, and it is ignored, but a
//! run that would write files writes nothing then; one a release of xtmd dropped fails, saying what
//! replaced it.

/// Each command's flags; one that takes a value takes it as the next argument or after `=`.
fn known(cmd: &str) -> &'static [&'static str] {
    // every command takes --format, --out and --help (page 1 of `xtmd`'s usage); one spelling each
    match cmd {
        "fmt" => &["--check", "--diff", "--stdout", "--stdin", "--format", "--out", "--help"],
        "parse" => &["--format", "--out", "--help"],
        "check" => &["--pack", "--config", "--format", "--out", "--help"],
        "compile" => &["--rir", "--core", "--pretty", "--pack", "--config", "--format", "--out", "--help"],
        "export" => &["--engine", "--pack", "--config", "--format", "--out", "--help"],
        "decompile" => &["--stdout", "--diff", "--check", "--format", "--out", "--help"],
        "md" => &["--pack", "--config", "--format", "--out", "--help"],
        "surface" => &["--check", "--diff", "--stdout", "--force", "--pack", "--config", "--format", "--out", "--help"],
        "migrate" => &[
            "--pack", "--config", "--engine", "--from", "--reader", "--answers", "--format", "--out", "--report", "--dry-run", "--check", "--diff", "--stdout", "--yes", "--allow-dirty", "--allow-errors", "--allow-todos", "--help",
        ],
        "lsp" => &["--help"],
        _ => &[],
    }
}

/// The flags that take a value, as the next argument or after `=`.
pub(crate) const VALUED: &[&str] = &["--format", "--out", "--pack", "--config", "--from", "--reader", "--answers", "--report"];

/// Whether a run of the command, with these flags, writes files the person has: fmt and surface
/// unless they only look, and migrate unless it only looks or prints.
pub(crate) fn writes(cmd: &str, args: &[String]) -> bool {
    let has = |f: &str| args.iter().any(|a| a == f);
    match cmd {
        "fmt" => !(has("--check") || has("--diff") || has("--stdout") || has("--stdin")),
        "surface" | "decompile" => !(has("--check") || has("--diff") || has("--stdout")),
        "migrate" => !(has("--dry-run") || has("--check") || has("--diff") || has("--stdout")),
        _ => false,
    }
}

/// A flag xtmd hasn't but is asked for: its command (`*` for every command), the flag, and why there is
/// none, or the one there is.
const ASKED_FOR: &[(&str, &str, &str)] = &[
    ("migrate", "--to", "a migration goes one way, to this xtmd's release and to the version of each pack in the build"),
    // one spelling for each flag: the short ones xtmd 0.3's development had, and the --json it had
    ("*", "-o", "use --out"),
    ("*", "-c", "use --config"),
    ("*", "-h", "use --help"),
    ("*", "--json", "use --format json"),
    ("migrate", "-y", "use --yes"),
];

/// A flag a release moved: its command, the flag, the release that moved it, and the one there is. Like
/// a command in `MOVED`, it answers in that release only; the next one removes it, by the table in
/// ".github/pre-commit/Pre Commit checks.md", which `.github/pre-commit/check.mjs` holds to this list.
pub(crate) const MOVED_FLAGS: &[(&str, &str, &str, &str)] = &[
    // build's, which export took over: the IR and the resolved IR are compile's
    ("export", "--rir", "0.4.0", "xtmd compile writes the resolved IR: xtmd compile page.xtmd --rir"),
    ("export", "--core", "0.4.0", "xtmd compile writes the resolved IR, core's files in it: xtmd compile page.xtmd --rir --core"),
    ("export", "--pretty", "0.4.0", "xtmd compile writes the IR: xtmd compile page.xtmd --pretty"),
];

/// The command's flags looked over: each one it doesn't know, warned about, and whether it can run;
/// one it hasn't but is asked for stops it, with what there is.
pub(crate) fn check(cmd: &str, args: &[String]) -> (Vec<String>, bool) {
    let known = known(cmd);
    let (mut unknown, mut ok) = (Vec::new(), true);
    for a in args.iter().filter(|a| is_flag(a)) {
        let name = a.split_once('=').map_or(a.as_str(), |(n, _)| n);
        if known.contains(&name) {
            continue;
        }
        let asked = ASKED_FOR.iter().find(|(c, f, _)| (*c == cmd || *c == "*") && *f == name).map(|(_, _, i)| *i);
        let moved = MOVED_FLAGS.iter().find(|(c, f, _, _)| *c == cmd && *f == name).map(|(_, _, _, i)| *i);
        match asked.or(moved) {
            Some(instead) => {
                eprintln!("error: xtmd {} has no {}: {}", cmd, name, instead);
                ok = false;
            }
            None => unknown.push(name.to_string()),
        }
    }
    (unknown, ok)
}

/// The warning for a flag the command doesn't know, with the one meant, when one is close.
pub(crate) fn warning(cmd: &str, flag: &str) -> String {
    let near = known(cmd).iter().copied().filter(|k| distance(k, flag) <= 2).min_by_key(|k| distance(k, flag));
    format!("xtmd {} has no flag {}, so it is ignored{}", cmd, flag, near.map_or(String::new(), |k| format!(" (did you mean {}?)", k)))
}

/// An argument that is a flag: `-x` or `--x…`, not `-` or a negative number.
pub(crate) fn is_flag(a: &str) -> bool {
    a.len() > 1 && a.starts_with('-') && a.trim_start_matches('-').chars().next().map_or(false, |c| c.is_ascii_alphabetic())
}

/// How many letters apart two flags are.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=b.len() {
            let here = row[j];
            row[j] = (row[j] + 1).min(row[j - 1] + 1).min(prev + usize::from(a[i - 1] != b[j - 1]));
            prev = here;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flag_asked_for_is_one_its_command_hasnt() {
        let moved = MOVED_FLAGS.iter().map(|(c, f, _, i)| (*c, *f, *i));
        for (cmd, flag, instead) in ASKED_FOR.iter().copied().chain(moved) {
            assert!(!known(cmd).contains(&flag), "xtmd {} takes {}", cmd, flag);
            assert!(!instead.is_empty());
        }
    }

    #[test]
    fn an_unknown_flag_is_ignored_and_one_asked_for_stops_the_command() {
        let args = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(check("migrate", &args(&["docs", "--yes=1,3", "--pack=kit", "--stdout", "--quiet"])), (vec!["--quiet".to_string()], true));
        assert!(!check("migrate", &args(&["docs", "--to", "0.3.0"])).1);
        assert_eq!(check("fmt", &args(&["a.xtmdpack", "--check", "-", "--format", "json", "--out", "x.txt"])), (vec![], true));
        assert!(!check("compile", &args(&["a.xtmd", "-o", "a.ir.json"])).1);
        assert!(!check("export", &args(&["a.xtmd", "--rir"])).1);
        assert!(!check("check", &args(&["a.xtmd", "-o", "report.txt"])).1);
    }

    #[test]
    fn a_near_miss_names_the_flag_meant() {
        assert_eq!(warning("fmt", "--chek"), "xtmd fmt has no flag --chek, so it is ignored (did you mean --check?)");
        assert_eq!(warning("fmt", "--quiet"), "xtmd fmt has no flag --quiet, so it is ignored");
    }
}
