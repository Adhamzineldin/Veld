//! What the typed runtime config does in generated code (`spec/runtime-config.md`, `spec/targets/rust.md`
//! section 15): the server entry with and without required paths, the rule output mode that comes
//! from the config, a bad config that fails when the handler is created with the whole list, the
//! clients that `clients_with` builds from the config, a client built without a URL, and the
//! readers.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use bookings::bookings::bookings_server::{BookingsHandler, BookingsService, PingRequest};
use bookings::runtime_config::{BookingsRuntime, BookingsServices, RulesSettings, ServiceSettings};
use orders::orders::orders::Report;
use orders::orders::orders_server::{OrdersHandler, OrdersService, ReportRequest};
use orders::runtime_config::{OrdersRuntime, RuleOutputMode, RulesSettings as OrdersRules};
use orders::veld::server::{Request, RequestContext};
use web::bookings::bookings_client::BookingsClient;
use web::runtime_config::{WebRuntime, clients, clients_with};
use web::veld::client::{ClientOptions, HttpRequest, HttpResponse, Transport};
use web::veld::{Error, block_on};

struct Pinger;

impl BookingsService for Pinger {
    async fn ping(&self, _request: PingRequest, _ctx: &bookings::veld::server::RequestContext) -> Result<i32, bookings::veld::Error> {
        Ok(1)
    }
}

/// An `Orders` implementation that breaks the invariant of its output when it is asked to.
struct Breaker;

impl OrdersService for Breaker {
    async fn report(&self, request: ReportRequest, _ctx: &RequestContext) -> Result<Report, orders::veld::Error> {
        Ok(Report { parts: vec![1, 2], total: if request.broken { 99 } else { 3 } })
    }
}

fn url(path: &str, text: &str) -> (String, String) {
    (path.to_string(), text.to_string())
}

fn bookings_config(iam: &str, payments: &str) -> BookingsRuntime {
    BookingsRuntime {
        rules: RulesSettings::default(),
        services: BookingsServices {
            iam: ServiceSettings { url: iam.to_string() },
            payments: ServiceSettings { url: payments.to_string() },
        },
    }
}

/// A bad config does not create a handler, and the error lists every problem, in path order.
fn a_bad_config_fails_when_the_handler_is_created() {
    let bad = bookings_config("ftp://iam", "");
    let Err(e) = BookingsHandler::plain(Pinger, &bad) else {
        panic!("a handler was created from a bad runtime config");
    };
    assert_eq!(
        e.to_string(),
        "invalid runtime config: services.iam.url must be an http or https URL; services.payments.url is missing"
    );
    assert_eq!(e.problems().len(), 2);
    assert!(BookingsHandler::plain(Pinger, &bookings_config("http://iam:3001/", "https://pay.example")).is_ok());
}

fn get(handler: &orders::veld::server::Handler<impl orders::veld::server::Endpoints>, broken: bool) -> u16 {
    let req = Request { method: "GET".into(), path: "/orders/report".into(), query: format!("broken={broken}"), ..Request::default() };
    let response = orders::veld::block_on(handler.handle(req)).expect("a route");
    response.status
}

/// The rule output mode is a setting: the default and `None` are `strict`.
fn the_output_mode_comes_from_the_runtime_config() {
    let strict = OrdersHandler::plain(Breaker, None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(get(&strict, false), 200);
    assert_eq!(get(&strict, true), 500, "strict is the default");
    let with = |output| {
        let rt = OrdersRuntime { rules: OrdersRules { output } };
        OrdersHandler::plain(Breaker, Some(&rt)).unwrap_or_else(|e| panic!("{e}"))
    };
    assert_eq!(get(&with(RuleOutputMode::Strict), true), 500);
    assert_eq!(get(&with(RuleOutputMode::Audit), true), 200, "audit sends it");
    assert_eq!(get(&with(RuleOutputMode::Off), true), 200, "off does not check");
    let parsed = OrdersRuntime::from_map(&BTreeMap::from([url("rules.output", "audit")])).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(get(&OrdersHandler::plain(Breaker, Some(&parsed)).unwrap_or_else(|e| panic!("{e}")), true), 200);
    let bad = OrdersRuntime::from_map(&BTreeMap::from([url("rules.output", "loud")]));
    assert_eq!(bad.err().map(|e| e.to_string()), Some("invalid runtime config: rules.output must be \"strict\", \"audit\" or \"off\"".to_string()));
    // `orders` has no required path: the config is an `Option`, and an empty one is the default.
    assert_eq!(OrdersRuntime::default().values().get("rules.output").map(String::as_str), Some("strict"));
}

/// A transport that records what it is asked to send.
/// A request as recorded: the URL and the headers.
type Sent = (String, Vec<(String, String)>);

#[derive(Clone, Default)]
struct Rec(Arc<Mutex<Vec<Sent>>>);

impl Transport for Rec {
    fn send(&self, req: HttpRequest) -> impl Future<Output = Result<HttpResponse, Error>> + Send {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((req.url, req.headers));
        std::future::ready(Err(Error::Transport("recorded".to_string())))
    }
}

fn web_config() -> WebRuntime {
    WebRuntime::from_map(&BTreeMap::from([url("services.bookings.url", "http://bookings:3000/"), url("services.iam.url", "https://iam.example//")]))
        .unwrap_or_else(|e| panic!("{e}"))
}

/// `clients_with` gives each client its URL (without the trailing `/`) and passes the options on.
fn the_clients_get_their_urls_and_the_options() {
    let rec = Rec::default();
    let all = clients_with(&web_config(), ClientOptions::new().header("x-trace", "1"), || rec.clone()).unwrap_or_else(|e| panic!("{e}"));
    assert!(block_on(all.bookings.ping()).is_err());
    assert!(block_on(all.users.who()).is_err());
    let sent = rec.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].0.starts_with("http://bookings:3000/bookings"), "{}", sent[0].0);
    assert!(sent[1].0.starts_with("https://iam.example/users"), "{}", sent[1].0);
    for (_, headers) in &sent {
        assert!(headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("x-trace") && v == "1"), "{headers:?}");
    }
    // The default transport builds without a request.
    assert!(clients(&web_config(), ClientOptions::new()).is_ok());
    // A config that does not pass the check builds nothing.
    let mut bad = web_config();
    bad.services.iam.url = "iam".to_string();
    let Err(e) = clients_with(&bad, ClientOptions::new(), Rec::default) else {
        panic!("clients were built from a bad runtime config");
    };
    assert_eq!(e.to_string(), "invalid runtime config: services.iam.url must be an http or https URL");
}

/// A client without a URL does not exist: the options must carry one.
fn a_client_needs_its_url() {
    let Err(e) = BookingsClient::new(Rec::default(), ClientOptions::new()) else {
        panic!("a client was built without a base URL");
    };
    assert!(e.to_string().contains("no base URL"), "{e}");
    assert!(BookingsClient::new(Rec::default(), ClientOptions::new().base_url("http://x")).is_ok());
}

/// `from_env_with` reads the variables the application names.
fn the_environment_reader_uses_the_names_of_the_application() {
    let env = BTreeMap::from([url("B_URL", "http://bookings:3000"), url("I_URL", "")]);
    let names = [("services.bookings.url", "B_URL"), ("services.iam.url", "I_URL")];
    let Err(e) = WebRuntime::from_env_with(&names, &|n| env.get(n).cloned()) else {
        panic!("an empty variable counts as set");
    };
    assert_eq!(e.to_string(), "invalid runtime config: services.iam.url is missing (environment variable I_URL is not set)");
}

fn main() {
    a_bad_config_fails_when_the_handler_is_created();
    the_output_mode_comes_from_the_runtime_config();
    the_clients_get_their_urls_and_the_options();
    a_client_needs_its_url();
    the_environment_reader_uses_the_names_of_the_application();
    println!("runtime behaviour ok");
}
