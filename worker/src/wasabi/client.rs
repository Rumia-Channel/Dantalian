use std::collections::BTreeMap;

use dantalian::{
    application::error::AppError,
    ports::object_storage::{
        MultipartObjectMetadata, MultipartPart, MultipartUploadStorage, ObjectMetadata,
        validate_object_key,
    },
};
use quick_xml::de::from_str;
use url::Url;
use worker::{Date, Fetch, Headers, Method, Request, RequestInit, Response, Result};

use super::{WasabiConfig, sigv4};

pub const UPLOAD_URL_TTL_SECONDS: u64 = 600;
pub const DOWNLOAD_URL_TTL_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct WasabiClient {
    config: WasabiConfig,
}

impl WasabiClient {
    pub fn new(config: WasabiConfig) -> Self {
        Self { config }
    }

    pub fn presigned_put_url(&self, key: &str, content_type: &str) -> Result<String, AppError> {
        self.presigned_url("PUT", key, Some(content_type), UPLOAD_URL_TTL_SECONDS)
    }

    pub fn presigned_get_url(&self, key: &str) -> Result<String, AppError> {
        self.presigned_url("GET", key, None, DOWNLOAD_URL_TTL_SECONDS)
    }
    pub fn presigned_head_url(&self, key: &str) -> Result<String, AppError> {
        self.presigned_url("HEAD", key, None, DOWNLOAD_URL_TTL_SECONDS)
    }

    pub async fn head_object(&self, key: &str) -> Result<ObjectMetadata, AppError> {
        let mut response = self.send(Method::Head, key, None).await?;
        let content_length = response
            .headers()
            .get("content-length")
            .map_err(storage_error)?
            .and_then(|value| value.parse::<u64>().ok());
        let content_type = response
            .headers()
            .get("content-type")
            .map_err(storage_error)?;
        let _ = response.text().await;
        Ok(ObjectMetadata {
            content_length,
            content_type,
        })
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), AppError> {
        self.send(Method::Delete, key, None).await.map(|_| ())
    }

    pub async fn put_object(
        &self,
        key: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), AppError> {
        let url = self.object_url(key)?;
        let now = now_seconds();
        let payload_hash = sigv4::sha256_hex(bytes);
        let mut canonical_headers = BTreeMap::new();
        canonical_headers.insert("content-type".to_string(), content_type.to_string());
        canonical_headers.insert("host".to_string(), host_header(&url)?);
        canonical_headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
        canonical_headers.insert("x-amz-date".to_string(), sigv4::amz_date(now));
        let signed = sigv4::sign_authorization(
            "PUT",
            url.path(),
            "",
            &canonical_headers,
            &payload_hash,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &self.config.region,
            now,
        );
        let request_headers = Headers::new();
        for (name, value) in &canonical_headers {
            request_headers.set(name, value).map_err(storage_error)?;
        }
        request_headers
            .set("authorization", &signed.authorization)
            .map_err(storage_error)?;
        let mut init = RequestInit::new();
        init.with_method(Method::Put)
            .with_headers(request_headers)
            .with_body(Some(worker::js_sys::Uint8Array::from(bytes).into()));
        let request = Request::new_with_init(url.as_str(), &init).map_err(storage_error)?;
        let mut response = Fetch::Request(request)
            .send()
            .await
            .map_err(storage_error)?;
        let status = response.status_code();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let _ = response.text().await;
        Err(AppError::Storage(format!(
            "Wasabi upload failed with status {status}"
        )))
    }

    async fn send(
        &self,
        method: Method,
        key: &str,
        content_type: Option<&str>,
    ) -> Result<Response, AppError> {
        let url = self.object_url(key)?;
        let now = now_seconds();
        let payload_hash = sigv4::sha256_hex(b"");
        let mut canonical_headers = BTreeMap::new();
        canonical_headers.insert("host".to_string(), host_header(&url)?);
        canonical_headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
        canonical_headers.insert("x-amz-date".to_string(), sigv4::amz_date(now));
        if let Some(content_type) = content_type {
            canonical_headers.insert("content-type".to_string(), content_type.to_string());
        }
        let signed = sigv4::sign_authorization(
            method.as_ref(),
            url.path(),
            "",
            &canonical_headers,
            &payload_hash,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &self.config.region,
            now,
        );
        let request_headers = Headers::new();
        for (name, value) in &canonical_headers {
            request_headers.set(name, value).map_err(storage_error)?;
        }
        request_headers
            .set("authorization", &signed.authorization)
            .map_err(storage_error)?;
        let mut init = RequestInit::new();
        init.with_method(method).with_headers(request_headers);
        let request = Request::new_with_init(url.as_str(), &init).map_err(storage_error)?;
        let mut response = Fetch::Request(request)
            .send()
            .await
            .map_err(storage_error)?;
        let status = response.status_code();
        if (200..300).contains(&status) {
            return Ok(response);
        }
        let _ = response.text().await;
        Err(match status {
            404 => AppError::NotFound,
            409 => AppError::Conflict("Wasabi object conflict".to_string()),
            _ => AppError::Storage(format!("Wasabi request failed with status {status}")),
        })
    }

    fn presigned_url(
        &self,
        method: &str,
        key: &str,
        content_type: Option<&str>,
        expires_in: u64,
    ) -> Result<String, AppError> {
        let url = self.object_url(key)?;
        let mut headers = BTreeMap::new();
        if let Some(content_type) = content_type {
            headers.insert("content-type".to_string(), content_type.to_string());
        }
        sigv4::presigned_url(
            url.as_str(),
            method,
            &headers,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &self.config.region,
            expires_in,
            now_seconds(),
        )
        .map_err(AppError::Storage)
    }

    fn object_url(&self, key: &str) -> Result<Url, AppError> {
        validate_object_key(key)?;
        let mut endpoint = Url::parse(&self.config.endpoint)
            .map_err(|error| AppError::Storage(format!("invalid Wasabi endpoint: {error}")))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(AppError::Validation(
                "Wasabi endpoint must be an HTTP URL without query or fragment".to_string(),
            ));
        }
        let base_path = endpoint.path().trim_end_matches('/');
        endpoint.set_path(&format!("{base_path}/{}/{}", self.config.bucket, key));
        Ok(endpoint)
    }
    async fn multipart_create(&self, key: &str, content_type: &str) -> Result<String, AppError> {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), content_type.to_string());
        let body = self
            .multipart_request(
                Method::Post,
                key,
                &[("uploads".to_string(), String::new())],
                headers,
                &[],
            )
            .await?;
        from_str::<InitiateMultipartResponse>(&body)
            .map(|response| response.upload_id)
            .map_err(|error| AppError::Storage(format!("invalid multipart init response: {error}")))
    }

    fn multipart_part_url(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
    ) -> Result<String, AppError> {
        validate_object_key(key)?;
        validate_upload_id(upload_id)?;
        validate_part_number(part_number)?;
        let url = self.object_url(key)?;
        let query = [
            ("partNumber".to_string(), part_number.to_string()),
            ("uploadId".to_string(), upload_id.to_string()),
        ];
        sigv4::presigned_url_with_query(
            url.as_str(),
            "PUT",
            &BTreeMap::new(),
            &query,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &self.config.region,
            UPLOAD_URL_TTL_SECONDS,
            now_seconds(),
        )
        .map_err(AppError::Storage)
    }

    async fn multipart_complete(
        &self,
        key: &str,
        upload_id: &str,
        parts: &[MultipartPart],
    ) -> Result<MultipartObjectMetadata, AppError> {
        validate_object_key(key)?;
        validate_upload_id(upload_id)?;
        if parts.is_empty() {
            return Err(AppError::Validation(
                "multipart completion requires at least one part".to_string(),
            ));
        }
        let mut xml = String::from("<CompleteMultipartUpload>");
        for part in parts {
            validate_part_number(part.part_number)?;
            if part.etag.is_empty() || part.etag.len() > 1024 {
                return Err(AppError::Validation("invalid multipart ETag".to_string()));
            }
            xml.push_str("<Part><PartNumber>");
            xml.push_str(&part.part_number.to_string());
            xml.push_str("</PartNumber><ETag>");
            xml.push_str(&xml_escape(&part.etag));
            xml.push_str("</ETag></Part>");
        }
        xml.push_str("</CompleteMultipartUpload>");
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/xml".to_string());
        self.multipart_request(
            Method::Post,
            key,
            &[("uploadId".to_string(), upload_id.to_string())],
            headers,
            xml.as_bytes(),
        )
        .await?;
        let metadata = self.head_object(key).await?;
        Ok(MultipartObjectMetadata {
            content_length: metadata.content_length.ok_or_else(|| {
                AppError::Storage("Wasabi multipart HEAD omitted content length".to_string())
            })?,
            content_type: metadata.content_type,
        })
    }

    async fn multipart_abort(&self, key: &str, upload_id: &str) -> Result<(), AppError> {
        validate_object_key(key)?;
        validate_upload_id(upload_id)?;
        self.multipart_request(
            Method::Delete,
            key,
            &[("uploadId".to_string(), upload_id.to_string())],
            BTreeMap::new(),
            &[],
        )
        .await
        .map(|_| ())
    }

    async fn multipart_request(
        &self,
        method: Method,
        key: &str,
        query: &[(String, String)],
        mut headers: BTreeMap<String, String>,
        body: &[u8],
    ) -> Result<String, AppError> {
        let url = self.object_url(key)?;
        let now = now_seconds();
        let payload_hash = sigv4::sha256_hex(body);
        headers.insert("host".to_string(), host_header(&url)?);
        headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
        headers.insert("x-amz-date".to_string(), sigv4::amz_date(now));
        let canonical_query = sigv4::canonical_query(query);
        let signed = sigv4::sign_authorization(
            method.as_ref(),
            url.path(),
            &canonical_query,
            &headers,
            &payload_hash,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &self.config.region,
            now,
        );
        let request_headers = Headers::new();
        for (name, value) in &headers {
            request_headers.set(name, value).map_err(storage_error)?;
        }
        request_headers
            .set("authorization", &signed.authorization)
            .map_err(storage_error)?;
        let mut request_url = url;
        request_url.set_query((!canonical_query.is_empty()).then_some(canonical_query.as_str()));
        let mut init = RequestInit::new();
        init.with_method(method).with_headers(request_headers);
        if !body.is_empty() {
            init.with_body(Some(worker::js_sys::Uint8Array::from(body).into()));
        }
        let request = Request::new_with_init(request_url.as_str(), &init).map_err(storage_error)?;
        let mut response = Fetch::Request(request)
            .send()
            .await
            .map_err(storage_error)?;
        let status = response.status_code();
        let text = response.text().await.map_err(storage_error)?;
        if (200..300).contains(&status) {
            return Ok(text);
        }
        Err(match status {
            404 => AppError::NotFound,
            409 => AppError::Conflict("Wasabi multipart conflict".to_string()),
            _ => AppError::Storage(format!(
                "Wasabi multipart request failed with status {status}"
            )),
        })
    }
}

impl dantalian::ports::object_storage::ObjectStorage for WasabiClient {
    async fn head(&self, key: &str) -> Result<ObjectMetadata, AppError> {
        self.head_object(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, AppError> {
        match self.head_object(key).await {
            Ok(_) => Ok(true),
            Err(AppError::NotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), AppError> {
        self.delete_object(key).await
    }

    async fn put_object(
        &self,
        key: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), AppError> {
        WasabiClient::put_object(self, key, content_type, bytes).await
    }

    async fn temporary_get_url(&self, key: &str) -> Result<String, AppError> {
        self.presigned_get_url(key)
    }
}
impl MultipartUploadStorage for WasabiClient {
    async fn create_multipart_upload(
        &self,
        key: &str,
        content_type: &str,
    ) -> Result<String, AppError> {
        self.multipart_create(key, content_type).await
    }

    fn presigned_upload_part_url(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
    ) -> Result<String, AppError> {
        self.multipart_part_url(key, upload_id, part_number)
    }

    async fn complete_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
        parts: &[MultipartPart],
    ) -> Result<MultipartObjectMetadata, AppError> {
        self.multipart_complete(key, upload_id, parts).await
    }

    async fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> Result<(), AppError> {
        self.multipart_abort(key, upload_id).await
    }
}

#[derive(Debug, serde::Deserialize)]
struct InitiateMultipartResponse {
    #[serde(rename = "UploadId")]
    upload_id: String,
}

fn validate_upload_id(upload_id: &str) -> Result<(), AppError> {
    if upload_id.is_empty() || upload_id.len() > 2_048 || upload_id.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Invalid multipart upload id".to_string(),
        ));
    }
    Ok(())
}

fn validate_part_number(part_number: u32) -> Result<(), AppError> {
    if !(1..=10_000).contains(&part_number) {
        return Err(AppError::Validation(
            "Multipart part number must be between 1 and 10000".to_string(),
        ));
    }
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

fn host_header(url: &Url) -> Result<String, AppError> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("Wasabi endpoint has no host".to_string()))?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn now_seconds() -> u64 {
    Date::now().as_millis() / 1_000
}

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(format!("Wasabi request error: {error}"))
}
