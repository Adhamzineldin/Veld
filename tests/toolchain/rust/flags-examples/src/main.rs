//! Serves the generated test handler of `tests/rules/endpoints/flags.veld.pending` on a free port
//! with stub application handlers, and prints `PORT=<n>`. The examples are sent to it with the flags
//! they turn on in `Veld-Test-Flags`.

use rooms_server::rooms::rooms::{Phase, Room, User};
use rooms_server::rooms::rooms_server::{
    ArchiveRoomFacts, ArchiveRoomRequest, BookRoomFacts, BookRoomRequest, PublishRoomFacts, PublishRoomRequest, RoomsHooks, RoomsService,
};
use rooms_server::rooms::rooms_test_server::{RoomsTestHandler, RoomsTestHooks, router};
use rooms_server::veld::Error;
use rooms_server::veld::flags::MemoryFlagProvider;
use rooms_server::veld::rules::Transition;
use rooms_server::veld::server::RequestContext;

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run) and one with `returns` brings its result.
struct Stub;

fn reached<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl RoomsService for Stub {
    async fn book_room(&self, _request: BookRoomRequest, _facts: BookRoomFacts, _ctx: &RequestContext) -> Result<Room, Error> {
        reached()
    }

    async fn publish_room(&self, _r: PublishRoomRequest, _f: PublishRoomFacts, _t: Transition<Phase>, _ctx: &RequestContext) -> Result<Room, Error> {
        reached()
    }

    async fn archive_room(&self, _r: ArchiveRoomRequest, _f: ArchiveRoomFacts, _t: Transition<Phase>, _ctx: &RequestContext) -> Result<Room, Error> {
        reached()
    }
}

/// The test handler never asks these hooks for a flag: it reads `Veld-Test-Flags`.
struct Hooks(MemoryFlagProvider);

impl RoomsHooks for Hooks {
    type Flags = MemoryFlagProvider;

    fn flags(&self) -> &MemoryFlagProvider {
        &self.0
    }

    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        None
    }
}

impl RoomsTestHooks for Hooks {}

#[tokio::main]
async fn main() {
    let app = router(RoomsTestHandler::create(Stub, Hooks(MemoryFlagProvider::default())));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
