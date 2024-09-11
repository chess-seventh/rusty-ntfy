#![allow(clippy::unused_io_amount)]
#![allow(clippy::implicit_hasher)]

pub mod notify;
pub mod tailscale;

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
    let local_server = tokio::task::spawn_blocking(tailscale::connect_to_tailscale_socket)
        .await
        .expect("Task panicked");

    let clients = tailscale::retrieve_peers(local_server.await).await;
    tailscale::prepare_peers(clients).await;
}
