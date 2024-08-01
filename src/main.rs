#![allow(clippy::unused_io_amount)]
use std::io::prelude::*;
use std::net::TcpStream;
use local_ip_address::list_afinet_netifas;
use once_cell::sync::OnceCell;
use configparser::ini::Ini;
use std::error::Error;


static SERVERS: OnceCell<Vec<Server>> = OnceCell::new();


#[derive(Debug, Clone)]
struct Server {
    ip: String,
    port: u16,
    name: String,
}

impl Server {
    fn new(ip: &str, port: u16, name: &str) -> Server {
        Server {
            ip: ip.to_string(),
            port,
            name: name.to_string(),
        }
    }

    fn connection_string(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}


#[tokio::main]
async fn main() {

    let nixos_servers = prep_servers();
    SERVERS.set(nixos_servers.clone()).unwrap();

    let _ = whereami().await.unwrap();

    for server in nixos_servers {
        match_server(server).await;
    };

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

async fn whereami() -> Result<String, reqwest::Error> {
    let pub_ip = get_pub_ip().await;
    let pub_ip2 = pub_ip.as_ref().unwrap();

    let client = reqwest::Client::new();
    let server = whoami();
    let msg = format!("Server: {server} public IP address is: {pub_ip2:}/32");

    let _ = client.post("https://ntfy.sh/c7-nixos-pings-d34d-b33f")
        .body(msg)
        .header("Title", "NixOS Pinging")
        .header("Priority", "urgent")
        .header("Tags", "rainbow")
        .send()
    .await?;

    pub_ip

}

fn prep_servers() -> Vec<Server> {
    match read_config() {
        Ok(servers) => servers,
        Err(e) => {
            panic!("Error reading config: {e}");
        },
    }
}

fn read_config() -> Result<Vec<Server>, Box<dyn Error>> {
    let mut config_ini = Ini::new();

    let _config_map = config_ini.load("rusty-ntfy.ini")?;
    let servers_names: Vec<String> = config_ini.get("servers", "names").unwrap().split_whitespace().map(std::string::ToString::to_string).collect();

    let vec_servers: Vec<Server> = servers_names.into_iter().map(|server| {
        let ip = config_ini.get(&server, "ip").expect("Could not read the server IP address from config");
        let port = config_ini.get(&server, "port").expect("Could not read the server PORT from config").parse::<u16>().expect("Could not parse the port into u16");
        Server::new(&ip, port, &server)
    }).collect();

    Ok(vec_servers)

}

async fn match_server(server: Server) {
    match connect_to_server(&server.clone()) {
        Ok(()) => {
            let _ = send_to_ntfy(server, "has been able to", "rainbow").await;
        },
        Err(_e) => {
            let _ =send_to_ntfy(server, "has NOT been able to", "skull").await;
        },
    }

}

fn connect_to_server(server: &Server) -> std::io::Result<()> {

    let mut stream = TcpStream::connect(server.connection_string())?;

    stream.write(&[1])?;
    stream.read(&mut [0; 128])?;
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

        let _ = client.post("https://ntfy.sh/c7-nixos-pings-d34d-b33f")
            .body(data_msg)
            .header("Title", "NIXOS Pinging")
            .header("Priority", "urgent")
            .header("Tags", emoji)
            .send()
        .await?;

        return Ok(())
    }

    Ok(())

}

