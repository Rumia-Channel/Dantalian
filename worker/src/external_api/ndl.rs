use super::{NdlBook, ndl_sru};
use crate::ndl_coordinator;
use dantalian::ndl_authority::{AUTHORITY_BATCH_SIZE, parse_authority_results};
use serde::Serialize;
use worker::{Env, Result};

pub(crate) async fn lookup_isbn(env: &Env, variants: &[String]) -> Result<Option<NdlBook>> {
    #[derive(Serialize)]
    struct IsbnRequest<'a> {
        isbn: &'a str,
    }
    let mut best: Option<NdlBook> = None;
    for variant in variants {
        let xml = ndl_coordinator::request(
            env,
            "/sru",
            serde_json::to_string(&IsbnRequest { isbn: variant })?,
            "NDL",
        )
        .await?;
        if let Some(book) = ndl_sru::parse_sru(&xml).map_err(worker::Error::RustError)? {
            if best
                .as_ref()
                .is_none_or(|previous| ndl_sru::score(&book) >= ndl_sru::score(previous))
            {
                best = Some(book);
            }
        }
    }
    if let Some(book) = best.as_mut() {
        enrich_authors(env, book).await;
    }
    Ok(best)
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
