use std::fmt::Write;

pub const SRU_ENDPOINT: &str = "https://ndlsearch.ndl.go.jp/api/sru";
pub const RECORD_SCHEMA: &str = "dcndl_v3";
pub const MAXIMUM_RECORDS: &str = "10";

/// Encode one ISBN condition. Clients issue distinct 10/13-digit conditions serially.
pub fn isbn_query(isbn: &str) -> String {
    let mut query = String::from("isbn=\"");
    for character in isbn.chars() {
        if matches!(character, '"' | '\\') {
            query.push('\\');
        }
        query
            .write_char(character)
            .expect("writing to a String cannot fail");
    }
    query.push('"');
    query
}

/// NDL uses a general SRU diagnostic URI even for a normal no-hit search.
/// Its API specification table 3-5 defines this message as an absent record.
pub fn check_sru_diagnostic(uri: &str, message: &str) -> Result<(), String> {
    if uri.trim() == "info:srw/diagnostic/1/1" && message.trim() == "Record does not exist" {
        Ok(())
    } else {
        Err(format!(
            "NDL SRU diagnostic {}: {}",
            uri.trim(),
            message.trim()
        ))
    }
}
