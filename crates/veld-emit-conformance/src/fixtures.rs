//! Small contracts, each exercising one capability, for the shared generator checks.

use veld_ir::Feature;

/// One contract and the feature it is meant to exercise (`None`: a plain contract).
#[derive(Debug, Clone, Copy)]
pub struct Fixture {
    pub name: &'static str,
    pub feature: Option<Feature>,
    pub source: &'static str,
}

const BASE: &str = r#"/// A greeting.
model Greeting {
  message: string
}

/// Greetings.
service Hello at "/hello" {
  /// Says hello.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
"#;

const UNIONS: &str = r#"model Card {
  number: string
}

model Transfer {
  iban: string
}

union Payment { card: Card, bank: Transfer }

service Pay at "/pay" {
  get "/{id}" find(id: uuid) -> Payment | NotFound
}
"#;

const CONSTRAINTS: &str = r#"model Signup {
  name: string @length(min: 1, max: 40)
  age: int @min(0) @max(150)
}

service Users at "/users" {
  post "/" signup(body: Signup) -> Signup
}
"#;

const AUTH: &str = r#"model Me {
  id: uuid
}

@auth(bearer)
service Account at "/me" {
  get "/" me() -> Me
}
"#;

const MIDDLEWARE: &str = r#"model Ping {
  ok: bool
}

@middleware(Audit)
service Health at "/health" {
  get "/" ping() -> Ping
}
"#;

const HEADERS: &str = r#"model Item {
  id: uuid
}

service Items at "/items" {
  get "/{id}" find(id: uuid, @header("X-Trace-Id") trace?: string) -> Item | NotFound
}
"#;

const SSE: &str = r#"model Tick {
  n: int
}

service Live at "/live" {
  get "/ticks" ticks() -> stream Tick
}
"#;

const WS: &str = r#"model Cmd {
  text: string
}

model Tick {
  n: int
}

service Live at "/live" {
  ws "/chat" chat(body: stream Cmd) -> stream Tick
}
"#;

const SSE_FULL: &str = r#"model Tick {
  n: int
}

error Gone 410 {
  reason: string
}

@auth(bearer)
service Feed at "/feed" {
  /// Path, query and header parameters, a declared error and auth on a stream.
  get "/{room}/events" events(room: string, @query since?: int, @header("Last-Event-ID") last?: string) -> stream Tick | Gone
}
"#;

const WS_FULL: &str = r#"model In {
  text: string @length(min: 1, max: 100)
}

model Out {
  text: string
}

error Denied 403 {
  why: string
}

@auth(apiKey(header: "X-Key"))
service Rooms at "/rooms" {
  /// Both directions, with path and query parameters and a declared error.
  ws "/{room}" talk(room: string, @query nick?: string, body: stream In) -> stream Out | Denied
  /// The server only sends.
  ws "/listen" listen() -> stream Out
  /// The client only sends.
  ws "/tell" tell(body: stream In) -> 204
}
"#;

const PAGE: &str = r#"model Item {
  id: uuid
}

service Items at "/items" {
  get "/" list() -> Page<Item>
}
"#;

const CONSTS: &str = r#"const MAX_ITEMS: int = 100

model Item {
  id: uuid
  qty: int = 1
}

service Items at "/items" {
  get "/" list() -> Item[]
}
"#;

const BYTES: &str = r#"model Blob {
  raw: bytes
}

service Blobs at "/blobs" {
  get "/{id}" find(id: uuid) -> Blob | NotFound
}
"#;

/// The plain contract first, then one per feature.
pub const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "plain",
        feature: None,
        source: BASE,
    },
    Fixture {
        name: "unions",
        feature: Some(Feature::Unions),
        source: UNIONS,
    },
    Fixture {
        name: "constraints",
        feature: Some(Feature::Constraints),
        source: CONSTRAINTS,
    },
    Fixture {
        name: "auth",
        feature: Some(Feature::Auth),
        source: AUTH,
    },
    Fixture {
        name: "middleware",
        feature: Some(Feature::Middleware),
        source: MIDDLEWARE,
    },
    Fixture {
        name: "headers",
        feature: Some(Feature::Headers),
        source: HEADERS,
    },
    Fixture {
        name: "streaming_sse",
        feature: Some(Feature::StreamingSse),
        source: SSE,
    },
    Fixture {
        name: "streaming_ws",
        feature: Some(Feature::StreamingWs),
        source: WS,
    },
    Fixture {
        name: "streaming_sse_full",
        feature: Some(Feature::StreamingSse),
        source: SSE_FULL,
    },
    Fixture {
        name: "streaming_ws_full",
        feature: Some(Feature::StreamingWs),
        source: WS_FULL,
    },
    Fixture {
        name: "page",
        feature: Some(Feature::Page),
        source: PAGE,
    },
    // `Feature::Consts` is never reported today: `features_used` only looks at declarations reachable
    // from endpoints, and nothing references a `const`. The fixture still guards against crashes.
    Fixture {
        name: "consts",
        feature: None,
        source: CONSTS,
    },
    Fixture {
        name: "bytes",
        feature: Some(Feature::Bytes),
        source: BYTES,
    },
    // A server refuses it with `E0801`; a client generates the same files as without the events.
    Fixture {
        name: "events",
        feature: Some(Feature::Events),
        source: crate::events::FIXTURE_SOURCE,
    },
];
