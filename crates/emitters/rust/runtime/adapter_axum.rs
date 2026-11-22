//! The axum adapter: one route per endpoint of a service, at its real path.
//!
//! ```ignore
//! let app = ::axum::Router::new().merge(crate::adapters::axum::router(handler));
//! ```
//!
//! Every route calls the same `Handler::invoke` pipeline as every other framework: auth,
//! middleware, validation, declared-error mapping. A trailing slash is accepted on every route.
//! A WebSocket endpoint is a `GET` route that upgrades the connection; the conversation is dropped
//! when the client goes away, so cleanup belongs in `Drop`.

use std::collections::BTreeMap;

use ::axum::Router;
use ::axum::body::Body;
use ::axum::extract::Request as AxumRequest;
use ::axum::http::{HeaderName, HeaderValue, StatusCode};
use ::axum::response::Response as AxumResponse;
use ::axum::routing::{MethodFilter, MethodRouter, on};

use crate::veld::server::{Body as VeldBody, Endpoints, Handler, Request};
use crate::veld::spec::{Kind, path_without_trailing_slash};
// @if ws_server
use std::future::Future;
use std::task::Poll;

use ::axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use ::axum::routing::get;

use crate::veld::server::{WsConversation, WsFrame};
use crate::veld::stream::next;
// @end

/// The routes of `handler`, to merge into your `axum::Router` or serve on their own.
pub fn router<E: Endpoints>(handler: Handler<E>) -> Router {
    let mut by_path: BTreeMap<String, MethodRouter> = BTreeMap::new();
    for ep in handler.endpoints().endpoints() {
        let name = ep.name;
        let h = handler.clone();
        let route = if ep.kind == Kind::Ws {
            // @if ws_server
            get(move |ws: WebSocketUpgrade, req: AxumRequest| {
                let h = h.clone();
                async move { upgrade(&h, ws, &req) }
            })
            // @end
            // @if !ws_server
            continue
            // @end
        } else {
            let Some(filter) = filter(ep.method) else {
                continue;
            };
            on(filter, move |req: AxumRequest| {
                let h = h.clone();
                async move { dispatch(&h, name, req).await }
            })
        };
        let path = path_without_trailing_slash(ep.path).to_string();
        let current = by_path.remove(&path).unwrap_or_default();
        by_path.insert(path, current.merge(route));
    }
    let mut router = Router::new();
    for (path, routes) in by_path {
        if path != "/" {
            router = router.route(&format!("{path}/"), routes.clone());
        }
        router = router.route(&path, routes);
    }
    router
}

fn filter(method: &str) -> Option<MethodFilter> {
    Some(match method {
        "GET" => MethodFilter::GET,
        "POST" => MethodFilter::POST,
        "PUT" => MethodFilter::PUT,
        "PATCH" => MethodFilter::PATCH,
        "DELETE" => MethodFilter::DELETE,
        "HEAD" => MethodFilter::HEAD,
        "OPTIONS" => MethodFilter::OPTIONS,
        _ => return None,
    })
}

const MAX_BODY: usize = 16 * 1024 * 1024;

fn request(parts: &::axum::http::request::Parts, body: Vec<u8>) -> Request {
    Request {
        method: parts.method.to_string(),
        path: parts.uri.path().to_string(),
        query: parts.uri.query().unwrap_or("").to_string(),
        headers: parts
            .headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_string(), v.to_string())))
            .collect(),
        body,
    }
}

async fn dispatch<E: Endpoints>(handler: &Handler<E>, name: &str, req: AxumRequest) -> AxumResponse {
    let (parts, body) = req.into_parts();
    let Ok(bytes) = ::axum::body::to_bytes(body, MAX_BODY).await else {
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    };
    match handler.invoke(name, request(&parts, bytes.to_vec())).await {
        Some(resp) => response(resp),
        None => status(StatusCode::NOT_FOUND),
    }
}

fn status(code: StatusCode) -> AxumResponse {
    let mut resp = AxumResponse::new(Body::empty());
    *resp.status_mut() = code;
    resp
}

/// A core response as an axum response.
pub fn response(resp: crate::veld::server::Response) -> AxumResponse {
    let body = match resp.body {
        VeldBody::Empty => Body::empty(),
        VeldBody::Bytes(b) => Body::from(b),
        // @if sse
        VeldBody::Sse(frames) => Body::from_stream(Frames(frames)),
        // @end
    };
    let mut out = AxumResponse::new(body);
    *out.status_mut() = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (k, v) in resp.headers {
        if let (Ok(k), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(&v)) {
            out.headers_mut().append(k, v);
        }
    }
    out
}

// @if sse
use crate::veld::stream::Stream;

struct Frames(crate::veld::stream::BoxStream<Vec<u8>>);

impl Stream for Frames {
    type Item = Result<Vec<u8>, std::convert::Infallible>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.as_mut().poll_next(cx).map(|item| item.map(Ok))
    }
}
// @end

// @if ws_server
fn upgrade<E: Endpoints>(handler: &Handler<E>, ws: WebSocketUpgrade, req: &AxumRequest) -> AxumResponse {
    let request = Request {
        method: "GET".into(),
        path: req.uri().path().to_string(),
        query: req.uri().query().unwrap_or("").to_string(),
        headers: req
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_string(), v.to_string())))
            .collect(),
        body: Vec::new(),
    };
    match handler.ws(request) {
        Some(conversation) => ws.on_upgrade(move |socket| serve(socket, conversation)),
        None => status(StatusCode::NOT_FOUND),
    }
}

/// What happened next on the socket.
enum Event {
    Received(Option<Result<Message, ::axum::Error>>),
    Frame(Option<WsFrame>),
}

/// Pumps one conversation: frames from the core to the socket, text frames from the socket into
/// the core. Binary frames close with 1003. The conversation is dropped when the loop ends.
async fn serve(mut socket: WebSocket, conversation: WsConversation) {
    let WsConversation { mut frames, input } = conversation;
    loop {
        let event = {
            let mut received = Box::pin(socket.recv());
            let mut frame = Box::pin(next(&mut frames));
            std::future::poll_fn(|cx| {
                if let Poll::Ready(f) = frame.as_mut().poll(cx) {
                    return Poll::Ready(Event::Frame(f));
                }
                if let Poll::Ready(m) = received.as_mut().poll(cx) {
                    return Poll::Ready(Event::Received(m));
                }
                Poll::Pending
            })
            .await
        };
        match event {
            Event::Frame(Some(WsFrame::Text(text))) => {
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            Event::Frame(Some(WsFrame::Close(code))) => {
                let _ = socket.send(Message::Close(Some(CloseFrame { code, reason: "".into() }))).await;
                finish_close(&mut socket).await;
                break;
            }
            Event::Received(Some(Ok(Message::Text(text)))) => input.push(text.to_string()),
            Event::Received(Some(Ok(Message::Binary(_)))) => {
                let _ = socket.send(Message::Close(Some(CloseFrame { code: 1003, reason: "".into() }))).await;
                finish_close(&mut socket).await;
                break;
            }
            // Ping and pong are answered by axum itself.
            Event::Received(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => {}
            Event::Frame(None) | Event::Received(Some(Ok(Message::Close(_)) | Err(_)) | None) => break,
        }
    }
    input.end();
}

/// Completes the closing handshake: after our close frame, reads until the peer's close frame (or
/// the connection ends), so unread data cannot turn the close into a reset that loses the frames
/// the peer has not read yet.
async fn finish_close(socket: &mut WebSocket) {
    let drain = async {
        while let Some(Ok(message)) = socket.recv().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    };
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), drain).await;
}
// @end
