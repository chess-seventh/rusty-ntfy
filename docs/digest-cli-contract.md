# rusty-ntfy `digest` CLI contract

The stable entrypoint jobs-forge (and any pure-shell caller) uses to push one
end-of-run digest. **This contract is stable** — jobs-forge's p8 baton 2 depends
on it verbatim. Additive flags are allowed; existing flags/semantics are not.

## Invocation

```text
rusty-ntfy digest --title <TITLE> [--body-file <PATH>] [--priority <P>] [--tags <T1,T2>]
```

| flag | required | meaning |
|------|----------|---------|
| `--title <TITLE>` | yes | notification title (ntfy `Title` header; Slack bolded first line) |
| `--body-file <PATH>` | no | multi-line body read from this file; **when omitted, the body is read from stdin** |
| `--priority <P>` | no | ntfy `Priority` (`1`..`5` or `min`/`low`/`default`/`high`/`urgent`) |
| `--tags <T1,T2>` | no | ntfy `Tags` (comma-separated emoji/keywords) |

Body on stdin (the common shell path):

```sh
printf 'scanned 12\ntailored 4\napplied 1\nfailed 0\n' \
  | rusty-ntfy digest --title 'jobs-forge run' --tags briefcase
```

## Transports & fan-out

The digest fans out to every **configured** transport:

- **ntfy** — always, from `[ntfy-topic]` in the ini (`RUSTY_NTFY_CONFIG` seam).
  Prefers a dedicated `digest_topic`; else reuses `topic_name` with the local
  `HOSTNAME` substituted.
- **Slack** — only when `[slack] webhook_pass_name` is set and the secret
  resolves (incoming-webhook, `{"text": "*title*\nbody"}`).

**Degrade-not-die per transport:** an unconfigured or unreachable transport is
skipped/logged, never sinking the others.

## Config (ini, referenced by `RUSTY_NTFY_CONFIG`)

```ini
[ntfy-topic]
# reused by the mesh-prober; digests prefer digest_topic when present
topic_name = c7-HOSTNAME-pings-d34d-b33f
digest_topic = c7-digests-d34d-b33f

[slack]
# a `pass` ENTRY NAME, never the URL itself — resolved at run time via `pass show`
webhook_pass_name = work/slack/jobs-digest
```

**Secrets by reference (hard rule):** only the `pass` entry *name* lives in
config/logs. The webhook URL is fetched at run time and is never stored,
committed, or logged.

## Exit codes

| code | meaning |
|------|---------|
| `0` | at least one transport delivered, or nothing was configured to send |
| `1` | total blackout — every attempted transport failed |
| `2` | bad invocation (missing `--title`, unknown flag, unreadable body-file) |

## Non-goals

Composing the digest string is the **caller's** job (jobs-forge builds
"scanned N, tailored M, applied K, failed J"); rusty-ntfy only delivers a
given title + body.
