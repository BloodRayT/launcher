use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub type Routes = HashMap<String, (u16, Vec<u8>)>;
/// Дополнительные заголовки ответа: путь -> готовая строка заголовка.
pub type Headers = HashMap<String, String>;

/// Минимальный HTTP-сервер для тестов. Остановка сервера имитирует пропажу сети.
pub struct TestServer {
    handle: JoinHandle<()>,
}

/// Занятый порт. Адрес известен заранее, потому что он нужен внутри JSON-ответов.
pub struct BoundPort {
    pub base: String,
    listener: TcpListener,
}

pub async fn bind_port() -> BoundPort {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    BoundPort {
        base: format!("http://{}", listener.local_addr().unwrap()),
        listener,
    }
}

impl BoundPort {
    pub fn serve(self, routes: Routes) -> TestServer {
        self.serve_with_headers(routes, Headers::new())
    }

    pub fn serve_with_headers(self, routes: Routes, headers: Headers) -> TestServer {
        let headers = Arc::new(headers);
        let listener = self.listener;
        let routes = Arc::new(routes);
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let routes = routes.clone();
                let headers = headers.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0u8; 1024];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => return,
                            Ok(read) => request.extend_from_slice(&buffer[..read]),
                        }
                    }
                    let text = String::from_utf8_lossy(&request);
                    let target = text.split_whitespace().nth(1).unwrap_or("/");
                    let path = target.split('?').next().unwrap_or(target);
                    let (status, body) = routes
                        .get(path)
                        .cloned()
                        .unwrap_or((404, b"not found".to_vec()));
                    let head = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",
                        body.len(),
                        headers
                            .get(path)
                            .map_or_else(String::new, |line| format!("{line}\r\n"))
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });
        TestServer { handle }
    }
}

impl TestServer {
    pub async fn stop(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}
