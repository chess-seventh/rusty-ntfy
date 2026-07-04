//! CLI parsing for the `digest` subcommand.
//!
//! Stable contract jobs-forge calls (documented in `docs/digest-cli-contract.md`):
//!
//! ```text
//! rusty-ntfy digest --title <TITLE> [--body-file <PATH>] [--priority <P>] [--tags <T1,T2>]
//! ```
//!
//! With no `--body-file`, the body is read from stdin.

use std::io::Read;
use std::slice::Iter;

#[derive(Debug, PartialEq, Eq)]
pub struct DigestArgs {
    pub title: String,
    pub body_file: Option<String>,
    pub priority: Option<String>,
    pub tags: Option<String>,
}

/// Parse the flags after `digest`. `--title` is required; the rest are optional.
pub fn parse_digest_args(args: &[String]) -> Result<DigestArgs, String> {
    let mut title = None;
    let mut body_file = None;
    let mut priority = None;
    let mut tags = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--title" => title = Some(next_val(&mut it, "--title")?),
            "--body-file" => body_file = Some(next_val(&mut it, "--body-file")?),
            "--priority" => priority = Some(next_val(&mut it, "--priority")?),
            "--tags" => tags = Some(next_val(&mut it, "--tags")?),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(DigestArgs {
        title: title.ok_or("--title is required")?,
        body_file,
        priority,
        tags,
    })
}

fn next_val(it: &mut Iter<String>, flag: &str) -> Result<String, String> {
    it.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

impl DigestArgs {
    /// Resolve the digest body: from `--body-file` when set, else drained from
    /// the provided reader (stdin in production, a fixture in tests).
    pub fn resolve_body(&self, stdin: &mut impl Read) -> Result<String, String> {
        if let Some(f) = &self.body_file {
            return std::fs::read_to_string(f).map_err(|e| format!("body-file {f}: {e}"));
        }
        let mut s = String::new();
        stdin.read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_all_flags() {
        let a = parse_digest_args(&args(&[
            "--title",
            "Run done",
            "--body-file",
            "/tmp/b",
            "--priority",
            "high",
            "--tags",
            "tada",
        ]))
        .unwrap();
        assert_eq!(
            a,
            DigestArgs {
                title: "Run done".into(),
                body_file: Some("/tmp/b".into()),
                priority: Some("high".into()),
                tags: Some("tada".into()),
            }
        );
    }

    #[test]
    fn title_is_required() {
        assert!(parse_digest_args(&args(&["--tags", "x"])).is_err());
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse_digest_args(&args(&["--title", "t", "--nope", "x"])).is_err());
    }

    #[test]
    fn flag_without_value_is_rejected() {
        assert!(parse_digest_args(&args(&["--title"])).is_err());
    }

    #[test]
    fn resolves_body_from_file() {
        let path = std::env::temp_dir().join(format!("rusty-ntfy-cli-{}.body", std::process::id()));
        std::fs::write(&path, "line1\nline2").unwrap();
        let a = DigestArgs {
            title: "t".into(),
            body_file: Some(path.to_string_lossy().into_owned()),
            priority: None,
            tags: None,
        };
        assert_eq!(
            a.resolve_body(&mut std::io::empty()).unwrap(),
            "line1\nline2"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn resolves_body_from_stdin_when_no_file() {
        let a = DigestArgs {
            title: "t".into(),
            body_file: None,
            priority: None,
            tags: None,
        };
        let mut stdin = "from stdin\nmore".as_bytes();
        assert_eq!(a.resolve_body(&mut stdin).unwrap(), "from stdin\nmore");
    }
}
