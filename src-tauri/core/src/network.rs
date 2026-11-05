use serde::Serialize;

/// Состояние сети разделено: при падении бекенда интернет и серверы Mojang могут работать.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Connectivity {
    pub internet: bool,
    pub backend: bool,
}

/// Адрес превью эфира: для идущего стрима сервер отдаёт картинку, иначе перенаправляет на заглушку.
pub fn twitch_preview_url(channel: &str) -> String {
    format!("https://static-cdn.jtvnw.net/previews-ttv/live_user_{channel}-320x180.jpg")
}

/// Определяет эфир без ключей Twitch. Клиенту нельзя следовать перенаправлениям.
pub async fn is_twitch_live(client: &reqwest::Client, preview_url: &str) -> bool {
    client
        .get(preview_url)
        .send()
        .await
        .is_ok_and(|response| response.status() == reqwest::StatusCode::OK)
}

pub async fn check_connectivity(
    client: &reqwest::Client,
    internet_probe_url: &str,
    backend_url: &str,
) -> Connectivity {
    let backend_probe = format!("{}/servers", backend_url.trim_end_matches('/'));
    let (internet, backend) = tokio::join!(
        client.head(internet_probe_url).send(),
        client.get(&backend_probe).send(),
    );
    Connectivity {
        // Любой ответ, даже ошибка HTTP, означает, что сеть работает.
        internet: internet.is_ok(),
        backend: backend.is_ok_and(|response| response.status().is_success()),
    }
}
