use std::collections::BTreeMap;

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use url::Url;

const SERVICE: &str = "s3";
const TERMINATOR: &str = "aws4_request";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedHeaders {
    pub authorization: String,
    pub signed_headers: String,
}

pub fn sha256_hex(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    to_hex(&digest)
}

pub fn canonical_uri(path: &str) -> String {
    let path = if path.is_empty() { "/" } else { path };
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte == b'/' || is_unreserved(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    if encoded.starts_with('/') {
        encoded
    } else {
        format!("/{encoded}")
    }
}

pub fn canonical_query(params: &[(String, String)]) -> String {
    let mut encoded = params
        .iter()
        .map(|(name, value)| (percent_encode(name), percent_encode(value)))
        .collect::<Vec<_>>();
    encoded.sort();
    encoded
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn canonical_headers(headers: &BTreeMap<String, String>) -> (String, String) {
    let mut normalized = BTreeMap::new();
    for (name, value) in headers {
        normalized.insert(name.to_ascii_lowercase(), normalize_header_value(value));
    }
    let canonical = normalized
        .iter()
        .map(|(name, value)| format!("{name}:{value}\n"))
        .collect::<String>();
    let signed = normalized.keys().cloned().collect::<Vec<_>>().join(";");
    (canonical, signed)
}

pub fn canonical_request(
    method: &str,
    path: &str,
    query: &str,
    headers: &BTreeMap<String, String>,
    payload_hash: &str,
) -> String {
    let (canonical_headers, signed_headers) = canonical_headers(headers);
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.to_ascii_uppercase(),
        canonical_uri(path),
        query,
        canonical_headers,
        signed_headers,
        payload_hash,
    )
}

pub fn amz_date(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let seconds = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

pub fn credential_scope(amz_timestamp: &str, region: &str) -> String {
    format!("{}/{}/{SERVICE}/{TERMINATOR}", &amz_timestamp[..8], region)
}

pub fn sign_authorization(
    method: &str,
    path: &str,
    query: &str,
    headers: &BTreeMap<String, String>,
    payload_hash: &str,
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    unix_seconds: u64,
) -> SignedHeaders {
    let timestamp = amz_date(unix_seconds);
    let (_, signed_headers) = canonical_headers(headers);
    let request = canonical_request(method, path, query, headers, payload_hash);
    let request_hash = sha256_hex(request.as_bytes());
    let scope = credential_scope(&timestamp, region);
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{request_hash}");
    let signature = signing_key(secret_access_key, &timestamp[..8], region, &string_to_sign);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key_id}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    SignedHeaders {
        authorization,
        signed_headers,
    }
}

pub fn presigned_url(
    base_url: &str,
    method: &str,
    headers: &BTreeMap<String, String>,
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    expires_in: u64,
    unix_seconds: u64,
) -> Result<String, String> {
    if expires_in == 0 || expires_in > 604_800 {
        return Err("presigned URL expiry must be between 1 and 604800 seconds".to_string());
    }
    let url = Url::parse(base_url).map_err(|error| format!("invalid Wasabi URL: {error}"))?;
    let host = host_header(&url)?;
    let mut signed_headers = headers.clone();
    signed_headers.insert("host".to_string(), host);
    let (_, signed_header_names) = canonical_headers(&signed_headers);
    let timestamp = amz_date(unix_seconds);
    let scope = credential_scope(&timestamp, region);
    let mut query = vec![
        (
            "X-Amz-Algorithm".to_string(),
            "AWS4-HMAC-SHA256".to_string(),
        ),
        (
            "X-Amz-Credential".to_string(),
            format!("{access_key_id}/{scope}"),
        ),
        ("X-Amz-Date".to_string(), timestamp.clone()),
        ("X-Amz-Expires".to_string(), expires_in.to_string()),
        ("X-Amz-SignedHeaders".to_string(), signed_header_names),
    ];
    let canonical_query_string = canonical_query(&query);
    let request = canonical_request(
        method,
        url.path(),
        &canonical_query_string,
        &signed_headers,
        "UNSIGNED-PAYLOAD",
    );
    let request_hash = sha256_hex(request.as_bytes());
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{timestamp}\n{scope}\n{request_hash}");
    let signature = signing_key(secret_access_key, &timestamp[..8], region, &string_to_sign);
    query.push(("X-Amz-Signature".to_string(), signature));
    Ok(format!("{base_url}?{}", canonical_query(&query)))
}

fn signing_key(secret: &str, date: &str, region: &str, string_to_sign: &str) -> String {
    let date_key = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, SERVICE.as_bytes());
    let signing_key = hmac_sha256(&service_key, TERMINATOR.as_bytes());
    let signature = hmac_sha256(&signing_key, string_to_sign.as_bytes());
    to_hex(&signature)
}

fn hmac_sha256(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts arbitrary keys");
    mac.update(value);
    mac.finalize().into_bytes().to_vec()
}

fn host_header(url: &Url) -> Result<String, String> {
    let host = url
        .host_str()
        .ok_or_else(|| "Wasabi endpoint has no host".to_string())?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn normalize_header_value(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if is_unreserved(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    encoded
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
}

fn to_hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(LOWER_HEX[(byte >> 4) as usize] as char);
        result.push(LOWER_HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = doy - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";
const HEX: &[u8; 16] = b"0123456789ABCDEF";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_formats_dates() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(amz_date(1_709_251_200), "20240301T000000Z");
    }

    #[test]
    fn canonicalizes_uri_query_and_headers() {
        assert_eq!(canonical_uri("/bucket/a b"), "/bucket/a%20b");
        assert_eq!(
            canonical_query(&[
                ("z".to_string(), "last".to_string()),
                ("a".to_string(), "two words".to_string()),
                ("a".to_string(), "first".to_string()),
            ]),
            "a=first&a=two%20words&z=last"
        );
        let headers = BTreeMap::from([
            ("Host".to_string(), " s3.example.test  ".to_string()),
            ("X-Amz-Meta-Test".to_string(), "one   two".to_string()),
        ]);
        assert_eq!(
            canonical_headers(&headers),
            (
                "host:s3.example.test\nx-amz-meta-test:one two\n".to_string(),
                "host;x-amz-meta-test".to_string()
            )
        );
    }

    #[test]
    fn builds_scope_signed_headers_and_presigned_query() {
        assert_eq!(
            credential_scope("20240301T000000Z", "us-east-1"),
            "20240301/us-east-1/s3/aws4_request"
        );
        let headers = BTreeMap::from([("content-type".to_string(), "image/jpeg".to_string())]);
        let url = presigned_url(
            "https://s3.example.test/bucket/images/id.jpg",
            "PUT",
            &headers,
            "AKID",
            "secret",
            "us-east-1",
            600,
            1_709_251_200,
        )
        .unwrap();
        assert!(url.contains("X-Amz-SignedHeaders=content-type%3Bhost"));
        assert!(url.contains("X-Amz-Credential=AKID%2F20240301%2Fus-east-1%2Fs3%2Faws4_request"));
        assert!(url.contains("X-Amz-Signature="));
        assert!(!url.contains("secret"));
    }
}
