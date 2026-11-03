use serde::Serialize;

/// Состояние сети разделено: при падении бекенда интернет и серверы Mojang могут работать.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Connectivity {
    pub internet: bool,
    pub backend: bool,
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
