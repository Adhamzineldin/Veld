//! The `reqwest` transport (async, needs a tokio runtime).

use super::client::{HttpRequest, HttpResponse, Transport};
use super::error::Error;
// @if sleep
use std::time::Duration;
// @end
// @if sse

use super::client::HttpStream;
use super::stream::unfold;
// @end
// @if ws_client
use super::client::{WsConn, WsEvent, WsSink};
// @end

/// A [`Transport`] on `reqwest`. Build it from your own `reqwest::Client` to share a pool,
/// proxies or TLS settings.
#[derive(Debug, Clone, Default)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// A transport on a default `reqwest::Client`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A transport on your client.
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        ReqwestTransport { client }
    }

    fn builder(&self, req: HttpRequest) -> Result<reqwest::RequestBuilder, Error> {
        let method = reqwest::Method::from_bytes(req.method.as_bytes()).map_err(|e| Error::Transport(e.to_string()))?;
        let mut b = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            b = b.header(k.as_str(), v.as_str());
        }
        Ok(match req.body {
            Some(body) => b.body(body),
            None => b,
        })
    }
}

fn failure(e: &reqwest::Error) -> Error {
    Error::Transport(e.to_string())
}

fn headers(resp: &reqwest::Response) -> Vec<(String, String)> {
    resp.headers().iter().filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_string(), v.to_string()))).collect()
}

impl Transport for ReqwestTransport {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, Error> {
        let timeout = req.timeout;
        let resp = self.builder(req)?.timeout(timeout).send().await.map_err(|e| failure(&e))?;
        let status = resp.status().as_u16();
        let headers = headers(&resp);
        let body = resp.bytes().await.map_err(|e| failure(&e))?.to_vec();
        Ok(HttpResponse { status, headers, body })
    }

    // @if sse
    async fn open(&self, req: HttpRequest) -> Result<HttpStream, Error> {
        let timeout = req.timeout;
        let sent = tokio::time::timeout(timeout, self.builder(req)?.send())
            .await
            .map_err(|_| Error::Transport("timed out while connecting".into()))?;
        let resp = sent.map_err(|e| failure(&e))?;
        let (status, headers) = (resp.status().as_u16(), headers(&resp));
        let body = unfold(resp, |mut resp: reqwest::Response| {
            Box::pin(async move {
                match resp.chunk().await {
                    Ok(Some(chunk)) => Some((Ok(chunk.to_vec()), resp)),
                    Ok(None) => None,
                    Err(e) => Some((Err(failure(&e)), resp)),
                }
            })
        });
        Ok(HttpStream { status, headers, body: Box::pin(body) })
    }
    // @end
    // @if sleep

    async fn sleep(&self, d: Duration) {
        tokio::time::sleep(d).await;
    }
    // @end

    // @if ws_client
    async fn connect_ws(&self, url: String, headers: Vec<(String, String)>, timeout: Duration) -> Result<WsConn, Error> {
        use futures_util::StreamExt;
        use tokio_tungstenite::tungstenite::Message;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};

        let mut request = url.into_client_request().map_err(|e| Error::Transport(e.to_string()))?;
        for (k, v) in &headers {
            if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                request.headers_mut().insert(k, v);
            }
        }
        let connected = tokio::time::timeout(timeout, tokio_tungstenite::connect_async(request))
            .await
            .map_err(|_| Error::Transport("timed out while connecting".into()))?;
        let (socket, _) = connected.map_err(|e| Error::Transport(e.to_string()))?;
        let (sink, stream) = socket.split();
        let events = unfold(stream, |mut stream| {
            Box::pin(async move {
                loop {
                    match stream.next().await {
                        Some(Ok(Message::Text(text))) => return Some((WsEvent::Text(text.to_string()), stream)),
                        Some(Ok(Message::Close(frame))) => {
                            return Some((WsEvent::Closed(frame.map_or(1005, |f| u16::from(f.code))), stream));
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => return Some((WsEvent::Failed(e.to_string()), stream)),
                        None => return None,
                    }
                }
            })
        });
        Ok(WsConn { sink: Box::new(Sink(sink)), events: Box::pin(events) })
    }
    // @end
}

// @if ws_client
struct Sink<S>(S);

impl<S> WsSink for Sink<S>
where
    S: futures_util::Sink<tokio_tungstenite::tungstenite::Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin + Send,
{
    fn send<'a>(&'a mut self, text: String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            futures_util::SinkExt::send(&mut self.0, tokio_tungstenite::tungstenite::Message::Text(text.into()))
                .await
                .map_err(|e| Error::Transport(e.to_string()))
        })
    }

    fn close<'a>(&'a mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let _ = futures_util::SinkExt::close(&mut self.0).await;
        })
    }
}
// @end
