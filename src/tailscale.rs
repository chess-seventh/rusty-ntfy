use local_ip_address::list_afinet_netifas;

use crate::notify::query_ntfy;
use crate::notify::send_to_ntfy;

use once_cell::sync::OnceCell;
static SERVERS: OnceCell<Vec<Server>> = OnceCell::new();

use super::Server;

use std::collections::HashMap;

use tailscale_localapi::{LocalApi, PeerStatus, UnixStreamClient};

use std::env;
use std::io::prelude::*;
use std::net::TcpStream;

pub async fn connect_to_tailscale_socket() -> LocalApi<UnixStreamClient> {
    let socket_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "/var/run/tailscale/tailscaled.sock".to_string());

    tailscale_localapi::LocalApi::new_with_socket_path(socket_path)
}

pub async fn retrieve_peers(
    client: tailscale_localapi::LocalApi<tailscale_localapi::UnixStreamClient>,
) -> HashMap<String, PeerStatus> {
    let client_status = client.status().await.unwrap();
    client_status.peer
}

pub async fn retrieve_self(
    client: tailscale_localapi::LocalApi<tailscale_localapi::UnixStreamClient>,
) -> Server {
    let client_status = client.status().await.unwrap();
    let self_client = client_status.self_status;

    Server {
        ip: self_client.tailscale_ips.first().unwrap().to_string(),
        port: get_proper_port(&self_client.hostname),
        name: self_client.hostname,
        online: self_client.online,
    }
}

pub async fn prepare_peers(peer: HashMap<String, tailscale_localapi::PeerStatus>) {
    let nixos_servers = get_peers(&peer);

    let _ = whereami(nixos_servers.clone()).await.unwrap();

    for server in nixos_servers {
        match_server(server).await;
    }
}

pub fn get_peers(peer: &HashMap<String, tailscale_localapi::PeerStatus>) -> Vec<Server> {
    peer.iter()
        .map(|host| Server {
            ip: host.1.tailscale_ips.first().unwrap().to_string(),
            port: get_proper_port(&host.1.hostname),
            name: host.1.hostname.clone(),
            online: host.1.online,
        })
        .collect::<Vec<Server>>()
}

pub fn get_proper_port(hostname: &str) -> u16 {
    if hostname.contains("bullwackies") {
        41234
    } else {
        22
    }
}

pub fn whoami() -> String {
    let local_ip = get_ip().unwrap();

    match SERVERS.get().unwrap().iter().find(|&s| s.ip == local_ip) {
        Some(server) => server.name.clone(),
        None => "unknown".to_string(),
    }
}

pub async fn get_pub_ip() -> Result<String, reqwest::Error> {
    reqwest::get("https://api.ipify.org").await?.text().await
}

pub async fn whereami(mut nixos_servers: Vec<Server>) -> Result<String, reqwest::Error> {
    let local_server = tokio::task::spawn_blocking(connect_to_tailscale_socket)
        .await
        .expect("Task panicked");

    let my_client = retrieve_self(local_server.await).await;

    nixos_servers.push(my_client);

    SERVERS.set(nixos_servers.clone()).unwrap();

    let pub_ip = get_pub_ip().await;
    let pub_ip2 = pub_ip.as_ref().unwrap();

    let client = reqwest::Client::new();
    let server = whoami();
    let msg = format!("Server: {server} public IP address is: {pub_ip2:}/32");

    query_ntfy(client, msg, "rock", &server).await?;

    pub_ip
}

pub async fn match_server(server: Server) {
    match connect_to_server(&server.clone()) {
        Ok(()) => {
            let _ = send_to_ntfy(server, "has been able to", "rainbow").await;
        }
        Err(_e) => {
            let _ = send_to_ntfy(server, "has NOT been able to", "skull").await;
        }
    }
}

pub fn connect_to_server(server: &Server) -> std::io::Result<()> {
    if server.online {
        let mut stream = TcpStream::connect(server.connection_string())?;

        stream.write(&[1])?;
        stream.read(&mut [0; 128])?;
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Server is offline",
        ))
    }
}

pub fn get_ip() -> Option<String> {
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
