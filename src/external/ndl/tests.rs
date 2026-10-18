use super::{authority, lookup_ndl, sru};
use crate::ndl_authority::build_authority_query;
use reqwest::{Client, Proxy};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, Instant, sleep};

#[tokio::test]
async fn ndl_requests_wait_between_failures_across_books_and_author_lookups() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let proxy = tokio::spawn(async move {
        let mut requests = tokio::task::JoinSet::new();
        for _ in 0..5 {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.spawn(async move {
                let mut buffer = [0; 2048];
                let bytes = stream.read(&mut buffer).await.unwrap();
                assert!(buffer[..bytes].starts_with(b"CONNECT "));
                let started = Instant::now();
                sleep(Duration::from_millis(100)).await;
                stream
                    .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
                (started, Instant::now())
            });
        }
        let mut times = Vec::new();
        while let Some(result) = requests.join_next().await {
            times.push(result.unwrap());
        }
        times.sort_by_key(|(started, _)| *started);
        times
    });
    let client = Client::builder()
        .proxy(Proxy::https(proxy_url).unwrap())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let query = build_authority_query(&["Pacing fixture author"]);
    let first_isbn = crate::amazon::isbn_lookup_variants("9784000000000");
    let second_isbn = crate::amazon::isbn_lookup_variants("9784000000017");
    let (first, second, author) = tokio::join!(
        lookup_ndl(&client, &first_isbn),
        lookup_ndl(&client, &second_isbn),
        authority::lookup(&client, &query, 1),
    );
    assert!(first.is_err());
    assert!(second.is_err());
    assert!(author.is_err());
    let times = proxy.await.unwrap();
    for pair in times.windows(2) {
        let idle = pair[1].0.saturating_duration_since(pair[0].1);
        assert!(
            idle >= Duration::from_secs(2),
            "NDL requests must wait at least two seconds after the previous response; observed {idle:?}"
        );
    }
}

#[test]
fn parses_v3_book_fields_without_confusing_identifiers_or_provider_metadata() {
    let xml = include_str!("../../../worker/tests/fixtures/ndl_v3.xml");
    let mut books = sru::parse_candidates(xml).unwrap();
    let book = books.pop().unwrap();
    assert_eq!(book.title, "NDL v3 fixture");
    assert_eq!(book.publisher.as_deref(), Some("Fixture Publisher"));
    assert_eq!(book.publish_date.as_deref(), Some("2026.9"));
    assert_eq!(book.jpno.as_deref(), Some("12345678"));
    assert_eq!(book.series_title.as_deref(), Some("Fixture Series"));
    assert_eq!(book.volume.as_deref(), Some("2"));
    assert_eq!(book.alternative.as_deref(), Some("Alternative title"));
    assert_eq!(book.authors[0].ndl_id.as_deref(), Some("00054222"));
    assert_eq!(book.authors[1].name, "森 鴎外");
    assert_eq!(book.authors[1].ndl_id, None);
    assert_eq!(book.authors[2].ndl_id, None);
}

#[test]
fn distinguishes_sru_diagnostics_from_an_empty_search() {
    assert!(sru::parse_candidates("<searchRetrieveResponse><diagnostics><diagnostic><uri>info:srw/diagnostic/1/1</uri></diagnostic></diagnostics></searchRetrieveResponse>").is_err());
    assert!(sru::parse_candidates("<response><recordData><dcndl:BibResource>").is_err());
    assert!(
        sru::parse_candidates(
            "<searchRetrieveResponse><numberOfRecords>0</numberOfRecords></searchRetrieveResponse>"
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn recognizes_ndl_missing_record_diagnostics_without_hiding_other_failures() {
    let xml = include_str!("../../../worker/tests/fixtures/ndl_no_records.xml");
    assert!(sru::parse_candidates(xml).unwrap().is_empty());
    assert!(
        sru::parse_candidates(&xml.replace("Record does not exist", "illegal query syntax"))
            .is_err()
    );
    assert!(
        sru::parse_candidates(&xml.replace("info:srw/diagnostic/1/1", "info:srw/diagnostic/1/2"))
            .is_err()
    );
    let mixed = xml.replace("</diagnostics>", "<diagnostic><uri>info:srw/diagnostic/1/1</uri><message>Server failure</message></diagnostic></diagnostics>");
    assert!(sru::parse_candidates(&mixed).is_err());
}
