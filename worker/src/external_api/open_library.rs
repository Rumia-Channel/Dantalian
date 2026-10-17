use super::{NdlAuthor, NdlBook};
use crate::ndl_coordinator;
use dantalian::open_library::parse_search_results;
use serde::Serialize;
use worker::{Env, Result};

pub(super) async fn lookup_isbn(env: &Env, variants: &[String]) -> Result<Option<NdlBook>> {
    #[derive(Serialize)]
    struct Input<'a> {
        isbns: &'a [String],
    }
    let json = ndl_coordinator::request(
        env,
        "/open-library",
        serde_json::to_string(&Input { isbns: variants })?,
        "Open Library",
    )
    .await?;
    Ok(parse_search_results(json.as_bytes(), variants)
        .map_err(worker::Error::RustError)?
        .map(|book| NdlBook {
            title: book.title,
            publisher: book.publisher,
            publish_date: book.publish_date,
            authors: book
                .authors
                .into_iter()
                .map(|name| NdlAuthor {
                    name,
                    ndl_id: None,
                    transcription: None,
                })
                .collect(),
            ..NdlBook::default()
        }))
}
