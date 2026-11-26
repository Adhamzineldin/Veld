//! The `ureq` transport: blocking. Its futures complete without waiting, so drive them with
//! `veld::block_on(client.get_thing(request))` or any executor.

use std::io::Read;

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
use super::client::WsConn;
// @end

/// A [`Transport`] on `ureq` 2. Build it from your own `ureq::Agent` for proxies or TLS settings.
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        UreqTransport { agent: ureq::Agent::new() }
    }
}

impl UreqTransport {
    /// A transport on a default agent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A transport on your agent.
    #[must_use]
    pub fn with_agent(agent: ureq::Agent) -> Self {
        UreqTransport { agent }
    }

    fn exchange(&self, req: &HttpRequest, streaming: bool) -> Result<ureq::Response, Error> {
        let mut r = self.agent.request(&req.method, &req.url);
        // A stream has no overall timeout: it ends when the server ends it.
        if !streaming {
            r = r.timeout(req.timeout);
        }
        for (k, v) in &req.headers {
            r = r.set(k, v);
        }
        let result = match &req.body {
            Some(body) => r.send_bytes(body),
            None => r.call(),
        };
        match result {
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => Ok(resp),
            Err(ureq::Error::Transport(t)) => Err(Error::Transport(t.to_string())),
        }
    }
}


fn headers(resp: &ureq::Response) -> Vec<(String, String)> {
    resp.headers_names()
        .into_iter()
        .filter_map(|n| resp.header(&n).map(|v| (n.clone(), v.to_string())))
        .collect()
}

impl Transport for UreqTransport {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, Error> {
        let resp = self.exchange(&req, false)?;
        let (status, headers) = (resp.status(), headers(&resp));
        let mut body = Vec::new();
        resp.into_reader().read_to_end(&mut body).map_err(|e| Error::Transport(e.to_string()))?;
        Ok(HttpResponse { status, headers, body })
    }

    // @if sse
    async fn open(&self, req: HttpRequest) -> Result<HttpStream, Error> {
        let resp = self.exchange(&req, true)?;
        let (status, headers) = (resp.status(), headers(&resp));
        let body = unfold(resp.into_reader(), |mut reader| {
            Box::pin(async move {
                let mut buf = vec![0u8; 4096];
                match reader.read(&mut buf) {
                    Ok(0) => None,
                    Ok(n) => {
                        buf.truncate(n);
                        Some((Ok(buf), reader))
                    }
                    Err(e) => Some((Err(Error::Transport(e.to_string())), reader)),
                }
            })
        });
        Ok(HttpStream { status, headers, body: Box::pin(body) })
    }
    // @end
    // @if sleep

    async fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
    // @end

    // @if ws_client
    async fn connect_ws(&self, _url: String, _headers: Vec<(String, String)>, _timeout: Duration) -> Result<WsConn, Error> {
        Err(Error::Transport("the ureq transport has no WebSocket client".into()))
    }
    // @end
}
