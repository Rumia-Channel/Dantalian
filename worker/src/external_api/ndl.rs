use super::{NdlBook, ndl_sru};
use crate::ndl_coordinator;
use dantalian::ndl_authority::{AUTHORITY_BATCH_SIZE, parse_authority_results};
use serde::Serialize;
use worker::{Env, Result};

pub(crate) async fn lookup_isbn(env: &Env, variants: &[String]) -> Result<Option<NdlBook>> {
    let mut failure = None;
    for variant in variants {
        match lookup_variant(env, variant).await {
            Ok(Some(mut book)) => {
                enrich_authors(env, &mut book).await;
                return Ok(Some(book));
            }
            Ok(None) => {}
            Err(error) => failure = Some(error),
        }
    }
    failure.map_or(Ok(None), Err)
}

async fn lookup_variant(env: &Env, isbn: &str) -> Result<Option<NdlBook>> {
    #[derive(Serialize)]
    struct IsbnRequest<'a> {
        isbn: &'a str,
    }
    let xml = ndl_coordinator::request(
        env,
        "/sru",
        serde_json::to_string(&IsbnRequest { isbn })?,
        "NDL",
    )
    .await?;
    ndl_sru::parse_sru(&xml).map_err(worker::Error::RustError)
}

async fn enrich_authors(env: &Env, book: &mut NdlBook) {
    let missing: Vec<usize> = book
        .authors
        .iter()
        .enumerate()
        .filter(|(_, author)| author.ndl_id.is_none() && !author.name.trim().is_empty())
        .map(|(index, _)| index)
        .collect();
    for batch in missing.chunks(AUTHORITY_BATCH_SIZE) {
        #[derive(Serialize)]
        struct NamesRequest<'a> {
            names: Vec<&'a str>,
        }
        let body = serde_json::to_string(&NamesRequest {
            names: batch
                .iter()
                .map(|&index| book.authors[index].name.as_str())
                .collect(),
        });
        let result = async {
            let json = ndl_coordinator::request(env, "/authorities", body?, "NDL").await?;
            parse_authority_results(&json, batch.len()).map_err(worker::Error::RustError)
        }
        .await;
        match result {
            Ok(ids) => {
                for (&index, id) in batch.iter().zip(ids) {
                    book.authors[index].ndl_id = id;
                }
            }
            Err(error) => worker::console_error!("NDL authority lookup failed: {error}"),
        }
    }
}
