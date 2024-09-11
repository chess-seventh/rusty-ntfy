use configparser::ini::Ini;
use std::env;
use std::error::Error;

use super::Server;
use crate::tailscale;

pub async fn send_to_ntfy(
    dest_server: Server,
    msg: &str,
    emoji: &str,
) -> Result<(), reqwest::Error> {
    let my_name = tailscale::whoami();

    if my_name.ne(&dest_server.name) {
        let data_msg = format!("{my_name} {msg} ping {0}", dest_server.name);
        let client = reqwest::Client::new();

        query_ntfy(client, data_msg, emoji, &my_name).await?;

        return Ok(());
    }

    Ok(())
}

pub async fn query_ntfy(
    client: reqwest::Client,
    data_msg: String,
    emoji: &str,
    my_name: &str,
) -> Result<(), reqwest::Error> {
    let url = prepare_url(my_name).unwrap();

    let _ = client
        .post(url)
        .body(data_msg)
        .header("Title", "NIXOS Pinging")
        .header("Tags", emoji)
        .send()
        .await?;
    Ok(())
}

fn prepare_url(server_name: &str) -> Result<String, Box<dyn Error>> {
    let mut config_ini = Ini::new();

    let config_file = format!(
        "{}/.config/rusty-ntfy/rusty-ntfy.ini",
        env::var("HOME").unwrap()
    );
    let _config_map = config_ini.load(config_file)?;
    let topic_name = config_ini.get("ntfy-topic", "topic_name").unwrap();
    let topic = topic_name.replace("HOSTNAME", server_name);

    Ok(format!("https://ntfy.sh/{topic}"))
}
