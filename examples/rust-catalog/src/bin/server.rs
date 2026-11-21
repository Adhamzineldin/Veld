use catalog_server::catalog::books::Book;
use catalog_server::catalog::catalog_server::{CatalogHandler, CatalogService, LookupRequest};
use catalog_server::prelude::NotFound;
use catalog_server::veld::server::RequestContext;
use catalog_server::veld::{DeclaredError, Error};
use tokio::net::TcpListener;

struct Shelf;

impl CatalogService for Shelf {
    async fn lookup(&self, request: LookupRequest, _: &RequestContext) -> Result<Book, Error> {
        let title = match request.isbn.as_str() {
            "9780141439518" => "Pride and Prejudice",
            "9780141439600" => "A Tale of Two Cities",
            _ => {
                return Err(NotFound {
                    resource: Some(request.isbn),
                }
                .into_error())
            }
        };
        Ok(Book {
            isbn: request.isbn,
            title: title.to_owned(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    println!("Catalog listening on http://{}", listener.local_addr()?);
    let handler = CatalogHandler::plain(Shelf);
    axum::serve(listener, catalog_server::adapters::axum::router(handler)).await
}
