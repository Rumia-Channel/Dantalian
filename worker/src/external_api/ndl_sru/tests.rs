use super::parse_sru;

#[test]
fn parses_v3_metadata_and_ignores_foreign_authority_ids() {
    let book = parse_sru(include_str!("../../../tests/fixtures/ndl_v3.xml"))
        .unwrap()
        .unwrap();
    assert_eq!(book.title, "NDL v3 fixture");
    assert_eq!(book.publish_date.as_deref(), Some("2026-09-NN"));
    assert_eq!(book.publisher.as_deref(), Some("Fixture Publisher"));
    assert_eq!(book.jpno.as_deref(), Some("12345678"));
    assert_eq!(book.series_title.as_deref(), Some("Fixture Series"));
    assert_eq!(book.authors[0].ndl_id.as_deref(), Some("00054222"));
    assert_eq!(book.authors[1].name, "森 鴎外");
    assert_eq!(book.authors[1].ndl_id, None);
    assert_eq!(book.authors[2].ndl_id, None);
}

#[test]
fn selects_the_richer_ndl_record_instead_of_the_first_provider_record() {
    let xml = r#"<response><records>
      <record><recordData><dcndl:BibResource rdf:about="https://ndlsearch.ndl.go.jp/books/R100000136-I1#material"><dcterms:title>Other provider</dcterms:title></dcndl:BibResource></recordData></record>
      <record><recordData><dcndl:BibResource rdf:about="https://ndlsearch.ndl.go.jp/books/R100000002-I2#material"><dcterms:title>NDL record</dcterms:title></dcndl:BibResource></recordData></record>
    </records></response>"#;
    assert_eq!(parse_sru(xml).unwrap().unwrap().title, "NDL record");
}

#[test]
fn rejects_sru_diagnostics_and_truncated_records_without_hiding_empty_results() {
    assert!(parse_sru("<response><diagnostics><diagnostic/></diagnostics></response>").is_err());
    assert!(parse_sru("<response><recordData><dcndl:BibResource>").is_err());
    assert!(
        parse_sru("<response><numberOfRecords>0</numberOfRecords></response>")
            .unwrap()
            .is_none()
    );
}

#[test]
fn recognizes_ndl_missing_record_diagnostics_without_hiding_other_failures() {
    let xml = include_str!("../../../tests/fixtures/ndl_no_records.xml");
    assert!(parse_sru(xml).unwrap().is_none());
    assert!(parse_sru(&xml.replace("Record does not exist", "illegal query syntax")).is_err());
    assert!(parse_sru(&xml.replace("info:srw/diagnostic/1/1", "info:srw/diagnostic/1/2")).is_err());
    let mixed = xml.replace("</diagnostics>", "<diagnostic><uri>info:srw/diagnostic/1/1</uri><message>Server failure</message></diagnostic></diagnostics>");
    assert!(parse_sru(&mixed).is_err());
}
