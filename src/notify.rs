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

// Config path: honour RUSTY_NTFY_CONFIG (set by the NixOS module to a
// sops-provided file) and fall back to the historic per-user location.
fn config_path() -> String {
    env::var("RUSTY_NTFY_CONFIG").unwrap_or_else(|_| {
        format!(
            "{}/.config/rusty-ntfy/rusty-ntfy.ini",
            env::var("HOME").unwrap()
        )
    })
}

fn topic_url_from(config_file: &str, server_name: &str) -> Result<String, Box<dyn Error>> {
    let mut config_ini = Ini::new();
    config_ini.load(config_file)?;
    let topic_name = config_ini
        .get("ntfy-topic", "topic_name")
        .ok_or("missing [ntfy-topic] topic_name in config")?;
    let topic = topic_name.replace("HOSTNAME", server_name);

    Ok(format!("https://ntfy.sh/{topic}"))
}

fn prepare_url(server_name: &str) -> Result<String, Box<dyn Error>> {
    topic_url_from(&config_path(), server_name)
}

#[cfg(test)]
mod tests {
    use super::topic_url_from;
    use std::fs;
    use std::io::Write;

    fn write_ini(suffix: &str, body: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("rusty-ntfy-{}-{suffix}.ini", std::process::id()));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn topic_url_substitutes_hostname_into_ntfy_url() {
        let path = write_ini(
            "ok",
            "[ntfy-topic]\ntopic_name = c7-HOSTNAME-pings-d34d-b33f\n",
        );
        let url = topic_url_from(path.to_str().unwrap(), "upsetter").unwrap();
        assert_eq!(url, "https://ntfy.sh/c7-upsetter-pings-d34d-b33f");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_topic_key_is_an_error() {
        let path = write_ini("empty", "[ntfy-topic]\n");
        assert!(topic_url_from(path.to_str().unwrap(), "upsetter").is_err());
        fs::remove_file(&path).ok();
    }
}
