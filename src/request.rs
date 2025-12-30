use std::time::Duration;

use reqwest::Client;

pub fn client() -> anyhow::Result<Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(5 * 60))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
