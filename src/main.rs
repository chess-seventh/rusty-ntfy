#![allow(clippy::unused_io_amount)]
#![allow(clippy::implicit_hasher)]

pub mod cli;
pub mod digest;
pub mod notify;
pub mod tailscale;

use digest::Digest;

#[derive(Debug, Clone)]
pub struct Server {
    ip: String,
    port: u16,
    name: String,
    online: bool,
}

impl Server {
    fn connection_string(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `rusty-ntfy digest ...` — the stable entrypoint jobs-forge calls. Any other
    // invocation runs the tailscale mesh-prober exactly as before (arg 1 stays the
    // optional socket path).
    if args.get(1).map(String::as_str) == Some("digest") {
        std::process::exit(run_digest(&args[2..]).await);
    }

    let local_server = tokio::task::spawn_blocking(tailscale::connect_to_tailscale_socket)
        .await
        .expect("Task panicked");

    let clients = tailscale::retrieve_peers(local_server.await).await;
    tailscale::prepare_peers(clients).await;
}

/// Drive the `digest` subcommand: parse flags, resolve the body (file or stdin),
/// fan out to the configured transports. Exit 0 unless every transport failed.
async fn run_digest(args: &[String]) -> i32 {
    let parsed = match cli::parse_digest_args(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rusty-ntfy digest: {e}");
            return 2;
        }
    };
    let body = match parsed.resolve_body(&mut std::io::stdin()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("rusty-ntfy digest: {e}");
            return 2;
        }
    };
    let d = Digest {
        title: parsed.title,
        body,
        priority: parsed.priority,
        tags: parsed.tags,
    };
    let transports = digest::transports_from_config();
    if transports.is_empty() {
        eprintln!("rusty-ntfy digest: no transports configured — nothing sent");
    }
    let outcome = digest::send_digest(&d, &transports).await;
    i32::from(!outcome.is_success())
}
