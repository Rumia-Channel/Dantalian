use super::{entity_id, parse_authority_results};
use serde_json::json;

fn row(slot: usize, id: &str) -> serde_json::Value {
    json!({
        "slot": {"type": "typed-literal", "value": slot.to_string()},
        "id": {"type": "uri", "value": format!("http://id.ndl.go.jp/auth/entity/{id}")}
    })
}

#[test]
fn preserves_author_positions_with_missing_and_reordered_results() {
    let body = json!({"results": {"bindings": [row(2, "00046801"), row(0, "00054222")]}});
    assert_eq!(
        parse_authority_results(&body.to_string(), 4).unwrap(),
        vec![
            Some("00054222".to_string()),
            None,
            Some("00046801".to_string()),
            None
        ]
    );
}

#[test]
fn conflicting_identities_stay_unresolved_even_after_more_rows() {
    let body = json!({"results": {"bindings": [
        row(0, "00054222"), row(0, "00046801"), row(0, "00054222"),
        row(1, "00046801"), row(1, "00046801")
    ]}});
    assert_eq!(
        parse_authority_results(&body.to_string(), 2).unwrap(),
        vec![None, Some("00046801".to_string())]
    );
}

#[test]
fn rejects_malformed_bindings_instead_of_assigning_to_another_author() {
    for binding in [
        row(3, "00054222"),
        json!({"slot": {"type": "literal", "value": "not-an-index"}, "id": {"type": "uri", "value": "http://id.ndl.go.jp/auth/entity/00054222"}}),
        json!({"slot": {"type": "literal", "value": "0"}, "id": {"type": "uri", "value": "https://other.example/auth/entity/00054222"}}),
        json!({"slot": {"type": "literal", "value": "0"}, "id": {"type": "literal", "value": "00054222"}}),
    ] {
        let body = json!({"results": {"bindings": [binding]}});
        assert!(parse_authority_results(&body.to_string(), 2).is_err());
    }
    assert!(parse_authority_results("{}", 2).is_err());
    assert!(parse_authority_results("not JSON", 2).is_err());
}

#[test]
fn recognizes_only_ndl_name_authority_uris() {
    assert_eq!(
        entity_id("http://id.ndl.go.jp/auth/entity/00054222"),
        Some("00054222")
    );
    assert_eq!(
        entity_id("https://id.ndl.go.jp/auth/ndlna/001234567"),
        Some("001234567")
    );
    for uri in [
        "https://ci.nii.ac.jp/author/DA00151899",
        "https://id.ndl.go.jp.other.example/auth/entity/00054222",
        "http://id.ndl.go.jp/auth/entity/",
        "http://id.ndl.go.jp/auth/entity/00054222?query=1",
        "http://id.ndl.go.jp/auth/ndlsh/00573385",
    ] {
        assert_eq!(entity_id(uri), None);
    }
}
