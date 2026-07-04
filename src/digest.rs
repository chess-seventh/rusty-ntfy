//! Transport-agnostic digest notifications.
//!
//! Generalises the notify path off the mesh-prober's fixed "NIXOS Pinging" ping
//! (`notify::send_to_ntfy`) into a reusable digest — a title + multi-line body +
//! optional priority/tags — fanned out to one or more transports (ntfy, Slack)
//! with **degrade-not-die** semantics: a transport that is unconfigured, or that
//! fails at send time, never sinks the others. Independent of tailscale internals
//! (no `tailscale::whoami` here), so jobs-forge can call it as a plain CLI.
//!
//! Secrets are held **by reference**: the Slack webhook is named in the ini as a
//! `pass` entry and fetched at run time — the URL is never stored, committed, or
//! logged.

use std::process::Command;

use configparser::ini::Ini;

use crate::notify::config_path;

/// A notification payload, transport-agnostic.
#[derive(Debug, Clone)]
pub struct Digest {
    pub title: String,
    pub body: String,
    pub priority: Option<String>,
    pub tags: Option<String>,
}

/// The transports a digest can fan out to. An enum (not a boxed trait object)
/// keeps the two known transports dependency-free — no `async-trait`, no boxing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Ntfy { url: String },
    Slack { webhook: String },
}

impl Transport {
    pub fn name(&self) -> &'static str {
        match self {
            Transport::Ntfy { .. } => "ntfy",
            Transport::Slack { .. } => "slack",
        }
    }

    async fn send(&self, client: &reqwest::Client, d: &Digest) -> Result<(), String> {
        match self {
            Transport::Ntfy { url } => {
                let mut req = client
                    .post(url)
                    .body(d.body.clone())
                    .header("Title", d.title.clone());
                if let Some(p) = &d.priority {
                    req = req.header("Priority", p.clone());
                }
                if let Some(t) = &d.tags {
                    req = req.header("Tags", t.clone());
                }
                req.send()
                    .await
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Transport::Slack { webhook } => {
                client
                    .post(webhook)
                    .header("Content-Type", "application/json")
                    .body(slack_payload(d))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }
}

/// The outcome of a fan-out: which transports were attempted and how each fared.
#[derive(Debug)]
pub struct DigestOutcome {
    pub results: Vec<(String, Result<(), String>)>,
}

impl DigestOutcome {
    pub fn delivered(&self) -> usize {
        self.results.iter().filter(|(_, r)| r.is_ok()).count()
    }

    pub fn failed(&self) -> usize {
        self.results.iter().filter(|(_, r)| r.is_err()).count()
    }

    /// Degrade-not-die: success as long as *something* got out (or there was
    /// nothing to send). Only a total blackout — every attempted transport
    /// failing — is a failure the caller should hear about.
    pub fn is_success(&self) -> bool {
        self.results.is_empty() || self.delivered() > 0
    }
}

/// Fan a digest out to every transport, isolating each: one failing transport is
/// logged and recorded, never propagated to the others.
pub async fn send_digest(d: &Digest, transports: &[Transport]) -> DigestOutcome {
    let client = reqwest::Client::new();
    let mut results = Vec::with_capacity(transports.len());
    for t in transports {
        let r = t.send(&client, d).await;
        match &r {
            Ok(()) => eprintln!("rusty-ntfy digest: {} delivered", t.name()),
            Err(e) => eprintln!("rusty-ntfy digest: {} FAILED: {e}", t.name()),
        }
        results.push((t.name().to_string(), r));
    }
    DigestOutcome { results }
}

/// Build the configured transports from the ini (`RUSTY_NTFY_CONFIG` seam) and
/// the secret store. ntfy comes from `[ntfy-topic]`; Slack is added only when
/// `[slack] webhook_pass_name` is set AND the secret resolves. A transport that
/// cannot be configured is simply absent (degrade-not-die), never an error.
pub fn transports_from_config() -> Vec<Transport> {
    transports_with(&config_path(), &pass_show)
}

fn transports_with(
    config_file: &str,
    pass_show: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<Transport> {
    let mut cfg = Ini::new();
    let mut out = Vec::new();
    if cfg.load(config_file).is_err() {
        eprintln!("rusty-ntfy digest: config {config_file} unreadable — no transports");
        return out;
    }
    if let Some(url) = ntfy_digest_url(&cfg) {
        out.push(Transport::Ntfy { url });
    }
    if let Some(slack) = slack_transport(&cfg, pass_show) {
        out.push(slack);
    }
    out
}

/// The ntfy topic URL for digests: prefer a dedicated `[ntfy-topic] digest_topic`
/// (so digests land in one fixed topic regardless of host); else reuse the mesh
/// `topic_name`, substituting the local hostname for `HOSTNAME`. No tailscale.
fn ntfy_digest_url(cfg: &Ini) -> Option<String> {
    if let Some(t) = cfg.get("ntfy-topic", "digest_topic") {
        return Some(format!("https://ntfy.sh/{t}"));
    }
    let t = cfg.get("ntfy-topic", "topic_name")?;
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "rusty-ntfy".to_string());
    Some(format!("https://ntfy.sh/{}", t.replace("HOSTNAME", &host)))
}

/// Resolve the Slack transport from config: the ini names a `pass` entry, whose
/// value is the incoming-webhook URL fetched at run time. The URL is never
/// stored or logged — only the reference name appears in config/logs.
fn slack_transport(
    cfg: &Ini,
    pass_show: &dyn Fn(&str) -> Result<String, String>,
) -> Option<Transport> {
    let name = cfg.get("slack", "webhook_pass_name")?;
    match pass_show(&name) {
        Ok(url) if !url.trim().is_empty() => Some(Transport::Slack {
            webhook: url.trim().to_string(),
        }),
        Ok(_) => {
            eprintln!("rusty-ntfy digest: slack pass entry '{name}' empty — skipping slack");
            None
        }
        Err(e) => {
            eprintln!("rusty-ntfy digest: slack secret '{name}' unresolved ({e}) — skipping slack");
            None
        }
    }
}

/// Fetch a secret by reference from `pass` (first line of `pass show <name>`).
fn pass_show(name: &str) -> Result<String, String> {
    let out = Command::new("pass")
        .arg("show")
        .arg(name)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("pass show exited {}", out.status));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout.lines().next().unwrap_or("").to_string())
}

/// The Slack incoming-webhook JSON payload: `{"text": "*title*\nbody"}`.
fn slack_payload(d: &Digest) -> String {
    let text = if d.body.is_empty() {
        d.title.clone()
    } else {
        format!("*{}*\n{}", d.title, d.body)
    };
    format!("{{\"text\":\"{}\"}}", json_escape(&text))
}

/// Minimal JSON string escaping — enough for the one `{"text": ...}` payload,
/// so no serde dependency is pulled in for a single field.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_ini(suffix: &str, body: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rusty-ntfy-digest-{}-{suffix}.ini",
            std::process::id()
        ));
        fs::write(&path, body).unwrap();
        path
    }

    fn d(title: &str, body: &str) -> Digest {
        Digest {
            title: title.to_string(),
            body: body.to_string(),
            priority: None,
            tags: None,
        }
    }

    #[test]
    fn json_escape_handles_quotes_newlines_backslashes() {
        assert_eq!(json_escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }

    #[test]
    fn slack_payload_bolds_title_and_joins_body() {
        assert_eq!(
            slack_payload(&d("Run done", "scanned 3\ntailored 2")),
            "{\"text\":\"*Run done*\\nscanned 3\\ntailored 2\"}"
        );
    }

    #[test]
    fn slack_payload_title_only_when_body_empty() {
        assert_eq!(slack_payload(&d("hi", "")), "{\"text\":\"hi\"}");
    }

    #[test]
    fn ntfy_digest_url_prefers_dedicated_digest_topic() {
        let path = write_ini("digest", "[ntfy-topic]\ndigest_topic = c7-digests-d34d\n");
        let mut cfg = Ini::new();
        cfg.load(path.to_str().unwrap()).unwrap();
        assert_eq!(
            ntfy_digest_url(&cfg),
            Some("https://ntfy.sh/c7-digests-d34d".to_string())
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn ntfy_digest_url_falls_back_to_topic_name_with_host() {
        let path = write_ini("fallback", "[ntfy-topic]\ntopic_name = c7-HOSTNAME-pings\n");
        let mut cfg = Ini::new();
        cfg.load(path.to_str().unwrap()).unwrap();
        std::env::set_var("HOSTNAME", "upsetter");
        assert_eq!(
            ntfy_digest_url(&cfg),
            Some("https://ntfy.sh/c7-upsetter-pings".to_string())
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn transports_ntfy_only_when_slack_unconfigured() {
        let path = write_ini("ntfyonly", "[ntfy-topic]\ndigest_topic = t\n");
        let ts = transports_with(path.to_str().unwrap(), &|_| {
            panic!("pass must not be consulted when [slack] is absent")
        });
        assert_eq!(
            ts,
            vec![Transport::Ntfy {
                url: "https://ntfy.sh/t".into()
            }]
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn transports_add_slack_from_pass_reference() {
        let path = write_ini(
            "both",
            "[ntfy-topic]\ndigest_topic = t\n[slack]\nwebhook_pass_name = work/slack/digest\n",
        );
        let ts = transports_with(path.to_str().unwrap(), &|name| {
            assert_eq!(name, "work/slack/digest");
            Ok("https://hooks.slack.com/services/XXX\n".to_string())
        });
        assert_eq!(
            ts,
            vec![
                Transport::Ntfy {
                    url: "https://ntfy.sh/t".into()
                },
                Transport::Slack {
                    webhook: "https://hooks.slack.com/services/XXX".into()
                },
            ]
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn transports_skip_slack_when_secret_unresolved() {
        let path = write_ini(
            "nosecret",
            "[ntfy-topic]\ndigest_topic = t\n[slack]\nwebhook_pass_name = missing/entry\n",
        );
        let ts = transports_with(path.to_str().unwrap(), &|_| Err("no such entry".into()));
        assert_eq!(
            ts,
            vec![Transport::Ntfy {
                url: "https://ntfy.sh/t".into()
            }]
        );
        fs::remove_file(&path).ok();
    }

    #[test]
    fn transports_empty_when_config_unreadable() {
        let ts = transports_with("/no/such/config.ini", &|_| Ok("x".into()));
        assert!(ts.is_empty());
    }

    #[test]
    fn outcome_success_when_any_delivered() {
        let o = DigestOutcome {
            results: vec![("ntfy".into(), Ok(())), ("slack".into(), Err("500".into()))],
        };
        assert!(o.is_success());
        assert_eq!(o.delivered(), 1);
        assert_eq!(o.failed(), 1);
    }

    #[test]
    fn outcome_failure_only_on_total_blackout() {
        let o = DigestOutcome {
            results: vec![("ntfy".into(), Err("down".into()))],
        };
        assert!(!o.is_success());
    }

    #[test]
    fn outcome_success_when_nothing_to_send() {
        let o = DigestOutcome { results: vec![] };
        assert!(o.is_success());
    }
}
