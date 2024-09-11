#![allow(clippy::unused_io_amount)]

use local_ip_address::list_afinet_netifas;

use once_cell::sync::OnceCell;
use tailscale_localapi::{LocalApi, PeerStatus, UnixStreamClient};

use std::collections::HashMap;
use std::env;
use std::io::prelude::*;
use std::net::TcpStream;

static SERVERS: OnceCell<Vec<Server>> = OnceCell::new();

#[derive(Debug, Clone)]
struct Server {
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
    let local_server = tokio::task::spawn_blocking(get_local_api_servers)
        .await
        .expect("Task panicked");

    let clients = retrieve_peers(local_server.await).await;
    prepare_peers(clients).await;
}

// async fn get_local_api_servers() -> HashMap<String, PeerStatus> {
async fn get_local_api_servers() -> LocalApi<UnixStreamClient> {
    let socket_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/var/run/tailscale/tailscaled.sock".to_string());

    tailscale_localapi::LocalApi::new_with_socket_path(socket_path)
}

async fn retrieve_peers(
    client: tailscale_localapi::LocalApi<tailscale_localapi::UnixStreamClient>,
) -> HashMap<String, PeerStatus> {
    let client_status = client.status().await.unwrap();
    client_status.peer
}

async fn retrieve_self(
    client: tailscale_localapi::LocalApi<tailscale_localapi::UnixStreamClient>,
) -> PeerStatus {
    let client_status = client.status().await.unwrap();
    client_status.self_status
}

async fn prepare_peers(peer: std::collections::HashMap<String, tailscale_localapi::PeerStatus>) {
    let nixos_servers = get_peers(&peer);

    let _ = whereami(nixos_servers.clone()).await.unwrap();

    for server in nixos_servers {
        match_server(server).await;
    }
}

fn get_peers(
    peer: &std::collections::HashMap<String, tailscale_localapi::PeerStatus>,
) -> Vec<Server> {
    peer.iter().map(peer_info).collect::<Vec<Server>>()
}

fn peer_info(host: (&String, &tailscale_localapi::PeerStatus)) -> Server {
    Server {
        ip: host.1.tailscale_ips.first().unwrap().to_string(),
        port: get_proper_port(&host.1.hostname),
        name: host.1.hostname.clone(),
        online: host.1.online,
    }
}

fn get_proper_port(hostname: &str) -> u16 {
    if hostname.contains("bullwackies") {
        41234
    } else {
        22
    }
}

fn whoami() -> String {
    let local_ip = get_ip().unwrap();

    match SERVERS.get().unwrap().iter().find(|&s| s.ip == local_ip) {
        Some(server) => server.name.clone(),
        None => "unknown".to_string(),
    }
}

async fn get_pub_ip() -> Result<String, reqwest::Error> {
    reqwest::get("https://api.ipify.org").await?.text().await
}

async fn whereami(mut nixos_servers: Vec<Server>) -> Result<String, reqwest::Error> {
    let local_server = tokio::task::spawn_blocking(get_local_api_servers)
        .await
        .expect("Task panicked");

    let self_client = retrieve_self(local_server.await).await;
    let my_client = Server {
        ip: self_client.tailscale_ips.first().unwrap().to_string(),
        port: get_proper_port(&self_client.hostname),
        name: self_client.hostname,
        online: self_client.online,
    };

    nixos_servers.push(my_client);

    SERVERS.set(nixos_servers.clone()).unwrap();

    let pub_ip = get_pub_ip().await;
    let pub_ip2 = pub_ip.as_ref().unwrap();

    let client = reqwest::Client::new();
    let server = whoami();
    let msg = format!("Server: {server} public IP address is: {pub_ip2:}/32");

    let _ = client
        .post(format!("https://ntfy.sh/c7-{server}-pings-d34d-b33f"))
        .body(msg)
        .header("Title", "NixOS Pinging")
        .header("Priority", "urgent")
        .header("Tags", "rainbow")
        .send()
        .await?;

    pub_ip
}

async fn match_server(server: Server) {
    match connect_to_server(&server.clone()) {
        Ok(()) => {
            let _ = send_to_ntfy(server, "has been able to", "rainbow").await;
        }
        Err(_e) => {
            let _ = send_to_ntfy(server, "has NOT been able to", "skull").await;
        }
    }
}

fn connect_to_server(server: &Server) -> std::io::Result<()> {
    if server.online {
        let mut stream = TcpStream::connect(server.connection_string())?;

        stream.write(&[1])?;
        stream.read(&mut [0; 128])?;
    }

    Ok(())
}

fn get_ip() -> Option<String> {
    let network_interfaces = list_afinet_netifas();

    if let Ok(network_interfaces) = network_interfaces {
        for (name, ip) in &network_interfaces {
            if name.contains("tailscale") && ip.is_ipv4() {
                return Some(ip.to_string());
            }
        }
    } else {
        panic!("Error getting network interfaces: {network_interfaces:?}");
    }
    None
}

async fn send_to_ntfy(dest_server: Server, msg: &str, emoji: &str) -> Result<(), reqwest::Error> {
    let my_name = whoami();

    if my_name != dest_server.name {
        let data_msg = format!("{my_name} {msg} ping {0}", dest_server.name);
        let client = reqwest::Client::new();

        let _ = client
            .post(format!("https://ntfy.sh/c7-{my_name}-pings-d34d-b33f"))
            .body(data_msg)
            .header("Title", "NIXOS Pinging")
            .header("Priority", "urgent")
            .header("Tags", emoji)
            .send()
            .await?;
        return Ok(());
    }

    Ok(())
}
