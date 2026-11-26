//! The Orders implementation behind the generated Rust server; mirrors
//! `tests/toolchain/ts/roundtrip.ts` and `tests/toolchain/java/ServerImpl.java`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use orders_server::iam::users::User;
use orders_server::orders::catalog::{Money, Status};
use orders_server::orders::orders::{Order, OutOfStock, QuotaExceeded};
use orders_server::orders::orders_server::*;
use orders_server::prelude::NotFound;
use orders_server::runtime_config::OrdersServerRuntime;
use orders_server::veld::server::{Principal, RequestContext};
use orders_server::veld::{DeclaredError, Error, Page};

use crate::check;

/// What the tests can learn about the server.
#[derive(Default)]
pub struct State {
    store: Mutex<BTreeMap<String, Order>>,
    next: AtomicUsize,
    pub thrown: Mutex<Vec<String>>,
    pub seen: Mutex<Vec<String>>,
    pub middleware_calls: AtomicUsize,
    pub requests: AtomicUsize,
}

impl State {
    fn id(&self) -> String {
        format!("00000000-0000-4000-8000-{:012x}", self.next.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn orders(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Order>> {
        self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The service.
#[derive(Clone)]
pub struct Impl(pub Arc<State>);

fn not_found(resource: Option<&str>) -> Error {
    NotFound { resource: resource.map(str::to_string) }.into_error()
}

impl OrdersService for Impl {
    async fn get_order(&self, request: GetOrderRequest, _ctx: &RequestContext) -> Result<Order, Error> {
        self.0.orders().get(&request.id).cloned().ok_or_else(|| not_found(Some("order")))
    }

    async fn create_order(&self, request: CreateOrderRequest, ctx: &RequestContext) -> Result<Order, Error> {
        check::equal(Some(&"u1".to_string()), ctx.principal::<String>(), "principal reaches the implementation");
        let body = request.body;
        if body.items.iter().any(|i| i.sku == "OUT-1") {
            return Err(OutOfStock { sku: "OUT-1".into() }.into());
        }
        if request.idempotency_key.as_deref() == Some("quota") {
            return Err(QuotaExceeded { limit: 5 }.into());
        }
        let note = request.idempotency_key.or(body.note);
        let order = Order {
            created_at: "2024-05-01T10:00:00Z".into(),
            id: self.0.id(),
            customer: User { id: body.customer_id, email: "ada@example.com".into(), display_name: "Ada".into() },
            items: body.items,
            status: Status::Pending,
            note,
            payment: body.payment,
            total: Money { amount: "10.50".into(), currency: "USD".into() },
            seq: 9_007_199_254_740_993,
            attributes: None,
            extra: None,
        };
        self.0.orders().insert(order.id.clone(), order.clone());
        Ok(order)
    }

    async fn list_orders(&self, request: ListOrdersRequest, _ctx: &RequestContext) -> Result<Page<Order>, Error> {
        let filter = request.query;
        let items: Vec<Order> = self
            .0
            .orders()
            .values()
            .filter(|o| filter.status.is_none_or(|s| s == o.status))
            .cloned()
            .collect();
        let limit = filter.limit.map_or(100, |l| usize::try_from(l).unwrap_or(0));
        if let Some(tags) = &filter.tag {
            self.0.seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!("tags={}", tags.join("|")));
        }
        let total = i64::try_from(items.len()).unwrap_or(0);
        let page: Vec<Order> = items.into_iter().take(limit).collect();
        let size = i32::try_from(page.len()).unwrap_or(0);
        Ok(Page { items: page, total, page: 1, size })
    }

    async fn search_orders(&self, request: SearchOrdersRequest, _ctx: &RequestContext) -> Result<Vec<Order>, Error> {
        let limit = request.limit.map_or("null".to_string(), |l| l.to_string());
        self.0.seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(format!("q={};limit={limit}", request.q));
        Ok(self.0.orders().values().filter(|o| o.note.as_deref().unwrap_or("").contains(&request.q)).cloned().collect())
    }

    async fn pay_order(&self, request: PayOrderRequest, _ctx: &RequestContext) -> Result<Order, Error> {
        let mut orders = self.0.orders();
        let order = orders.get_mut(&request.id).ok_or_else(|| not_found(None))?;
        order.status = Status::Paid;
        order.payment = Some(request.body);
        Ok(order.clone())
    }

    async fn cancel_order(&self, request: CancelOrderRequest, _ctx: &RequestContext) -> Result<(), Error> {
        self.0.orders().remove(&request.id).map(|_| ()).ok_or_else(|| not_found(Some("order")))
    }

    async fn health(&self, _request: HealthRequest, _ctx: &RequestContext) -> Result<(), Error> {
        Ok(())
    }

    async fn boom(&self, _request: BoomRequest, _ctx: &RequestContext) -> Result<Order, Error> {
        Err(Error::other("secret internal detail"))
    }
}

/// The hooks: bearer `good` is user `u1`, the rate limiter counts.
pub struct Hooks(pub Arc<State>);

impl OrdersHooks for Hooks {
    async fn auth_bearer(&self, token: &str, _ctx: &RequestContext) -> Option<Principal> {
        (token == "good").then(|| Principal::new("u1".to_string()))
    }

    async fn middleware_rate_limit(&self, _ctx: &RequestContext) -> Result<(), Error> {
        self.0.middleware_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.0.thrown.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(err.to_string());
    }
}

/// The generated router around this implementation, counting every request.
pub fn app(state: &Arc<State>) -> axum::Router {
    // The server consumes `iam`: its runtime config has the one required URL. The scenario never
    // calls the identity service, so the address only has to be a valid URL.
    let runtime = OrdersServerRuntime::from_map(&BTreeMap::from([(
        "services.iam.url".to_string(),
        "http://127.0.0.1:1".to_string(),
    )]))
    .unwrap_or_else(|e| panic!("{e}"));
    let handler = OrdersHandler::create(Impl(Arc::clone(state)), Hooks(Arc::clone(state)), &runtime)
        .unwrap_or_else(|e| panic!("{e}"));
    let counted = Arc::clone(state);
    orders_server::adapters::axum::router(handler).layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let counted = Arc::clone(&counted);
            async move {
                counted.requests.fetch_add(1, Ordering::SeqCst);
                next.run(req).await
            }
        },
    ))
}
