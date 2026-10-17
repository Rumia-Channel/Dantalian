use dantalian::ndl_authority::{AUTHORITY_BATCH_SIZE, AUTHORITY_ENDPOINT, build_authority_query};
use dantalian::ndl_search::{MAXIMUM_RECORDS, RECORD_SCHEMA, SRU_ENDPOINT, isbn_query};
use dantalian::open_library::{
    SEARCH_ENDPOINT, SEARCH_FIELDS, SEARCH_LIMIT, search_query, user_agent,
};
use futures_util::future::{Either, select};
use futures_util::lock::{Mutex, MutexGuard};
use futures_util::pin_mut;
use serde::Deserialize;
use std::time::Duration;
use url::{Url, form_urlencoded};
use worker::wasm_bindgen;
use worker::{
    AbortController, Delay, DurableObject, Env, Fetch, Headers, Method, Request, RequestInit,
    Response, Result, State, durable_object,
};

const INTERVAL: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[durable_object(fetch)]
pub struct NdlCoordinator {
    gate: Mutex<()>,
    open_library_user_agent: std::borrow::Cow<'static, str>,
}

impl DurableObject for NdlCoordinator {
    fn new(_state: State, env: Env) -> Self {
        let contact = env
            .var("OPEN_LIBRARY_CONTACT")
            .ok()
            .map(|value| value.to_string());
        Self {
            gate: Mutex::new(()),
            open_library_user_agent: user_agent(contact.as_deref()),
        }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        if req.method() != Method::Post {
            return Response::error("Method not allowed", 405);
        }
        let (outbound, label) = match req.path().as_str() {
            "/sru" => {
                #[derive(Deserialize)]
                struct Input {
                    isbn: String,
                }
                let input: Input = req.json().await?;
                let mut url = Url::parse(SRU_ENDPOINT)?;
                url.query_pairs_mut()
                    .append_pair("operation", "searchRetrieve")
                    .append_pair("version", "1.2")
                    .append_pair("recordSchema", RECORD_SCHEMA)
                    .append_pair("onlyBib", "true")
                    .append_pair("maximumRecords", MAXIMUM_RECORDS)
                    .append_pair("startRecord", "1")
                    .append_pair("recordPacking", "xml")
                    .append_pair("query", &isbn_query(&input.isbn));
                (ndl_request(url.as_str(), None)?, "NDL")
            }
            "/authorities" => {
                #[derive(Deserialize)]
                struct Input {
                    names: Vec<String>,
                }
                let input: Input = req.json().await?;
                if input.names.is_empty() || input.names.len() > AUTHORITY_BATCH_SIZE {
                    return Response::error("Invalid authority batch size", 400);
                }
                let names: Vec<&str> = input.names.iter().map(String::as_str).collect();
                let body = form_urlencoded::Serializer::new(String::new())
                    .append_pair("query", &build_authority_query(&names))
                    .append_pair("format", "json")
                    .finish();
                (ndl_request(AUTHORITY_ENDPOINT, Some(body))?, "NDL")
            }
            "/open-library" => {
                #[derive(Deserialize)]
                struct Input {
                    isbns: Vec<String>,
                }
                let input: Input = req.json().await?;
                if input.isbns.is_empty() || input.isbns.len() > 2 {
                    return Response::error("Invalid ISBN variant count", 400);
                }
                let mut url = Url::parse(SEARCH_ENDPOINT)?;
                url.query_pairs_mut()
                    .append_pair("q", &search_query(&input.isbns))
                    .append_pair("fields", SEARCH_FIELDS)
                    .append_pair("limit", SEARCH_LIMIT);
                let headers = Headers::new();
                headers.set("User-Agent", self.open_library_user_agent.as_ref())?;
                headers.set("Accept", "application/json")?;
                let mut init = RequestInit::new();
                init.with_headers(headers);
                (Request::new_with_init(url.as_str(), &init)?, "Open Library")
            }
            _ => return Response::error("Not found", 404),
        };
        let _guard = self.acquire().await;
        match fetch_complete(outbound, label).await {
            Ok(response) => Ok(response),
            Err(error) => Response::error(format!("{label} request failed: {error}"), 502),
        }
    }
}

impl NdlCoordinator {
    async fn acquire(&self) -> MutexGuard<'_, ()> {
        let guard = loop {
            if let Some(guard) = self.gate.try_lock() {
                break guard;
            }
            // A timer belongs to this invocation; a cross-invocation mutex waker
            // alone would be treated as a hung request by the Workers runtime.
            Delay::from(INTERVAL).await;
        };
        Delay::from(INTERVAL).await;
        guard
    }
}

fn ndl_request(url: &str, body: Option<String>) -> Result<Request> {
    let headers = Headers::new();
    headers.set(
        "User-Agent",
        concat!(
            "Dantalian/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Rumia-Channel/Dantalian)"
        ),
    )?;
    let mut init = RequestInit::new();
    if let Some(body) = body {
        headers.set("Accept", "application/sparql-results+json")?;
        headers.set("Content-Type", "application/x-www-form-urlencoded")?;
        init.with_method(Method::Post).with_body(Some(body.into()));
    } else {
        headers.set("Accept", "application/xml")?;
    }
    init.with_headers(headers);
    Request::new_with_init(url, &init)
}

struct AbortOnDrop(Option<AbortController>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(controller) = self.0.take() {
            controller.abort();
        }
    }
}

async fn fetch_complete(request: Request, label: &str) -> Result<Response> {
    let controller = AbortController::default();
    let signal = controller.signal();
    let _abort = AbortOnDrop(Some(controller));
    let fetch = async {
        let mut response = Fetch::Request(request).send_with_signal(&signal).await?;
        let status = response.status_code();
        let headers = Headers::new();
        if let Some(content_type) = response.headers().get("Content-Type")? {
            headers.set("Content-Type", &content_type)?;
        }
        let bytes = response.bytes().await?;
        Ok(Response::from_bytes(bytes)?
            .with_status(status)
            .with_headers(headers))
    };
    let timeout = Delay::from(REQUEST_TIMEOUT);
    pin_mut!(fetch, timeout);
    match select(fetch, timeout).await {
        Either::Left((result, _)) => result,
        Either::Right((_, pending)) => {
            drop(pending);
            Err(worker::Error::RustError(format!(
                "{label} request and response body timed out after 10 seconds"
            )))
        }
    }
}

pub(crate) async fn request(env: &Env, path: &str, body: String, label: &str) -> Result<String> {
    let namespace = env.durable_object("NDL_COORDINATOR")?;
    let stub = namespace.id_from_name("ndl")?.get_stub()?;
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into()));
    let mut response = stub
        .fetch_with_request(Request::new_with_init(
            &format!("https://ndl.internal{path}"),
            &init,
        )?)
        .await?;
    if !(200..300).contains(&response.status_code()) {
        return Err(worker::Error::RustError(format!(
            "{label} request failed: HTTP {}",
            response.status_code(),
        )));
    }
    response.text().await
}
