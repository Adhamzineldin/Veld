use std::process::ExitCode;

use catalog_client::catalog::catalog_client::{CatalogClient, LookupRequest};
use catalog_client::prelude::NotFound;
use catalog_client::veld::client::ClientOptions;
use catalog_client::veld::{block_on, DeclaredError, Error};

fn main() -> Result<ExitCode, Error> {
    let Some(isbn) = std::env::args().nth(1) else {
        eprintln!("Usage: lookup <ISBN>");
        return Ok(ExitCode::from(2));
    };
    let url = std::env::var("CATALOG_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".into());
    let client = CatalogClient::ureq(ClientOptions::new().base_url(url))?;
    match block_on(client.lookup(LookupRequest { isbn })) {
        Ok(book) => {
            println!("{} ({})", book.title, book.isbn);
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            if let Some(missing) = NotFound::from_error(&error) {
                eprintln!(
                    "No book found for ISBN {}",
                    missing.resource.as_deref().unwrap_or("unknown")
                );
                Ok(ExitCode::FAILURE)
            } else {
                Err(error)
            }
        }
    }
}
