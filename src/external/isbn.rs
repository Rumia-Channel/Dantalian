use crate::{amazon, db::NewBook};
use reqwest::Client;

pub struct IsbnLookup {
    pub book: NewBook,
    pub source: &'static str,
}

#[derive(Debug)]
pub enum IsbnLookupError {
    Ndl(String),
    OpenLibrary(String),
}

impl std::fmt::Display for IsbnLookupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ndl(message) | Self::OpenLibrary(message) => formatter.write_str(message),
        }
    }
}

pub async fn lookup_isbn(
    client: &Client,
    isbn: &str,
    images_dir: &str,
) -> Result<Option<IsbnLookup>, IsbnLookupError> {
    let isbn_variants = amazon::isbn_lookup_variants(isbn);
    let (metadata, source) = match super::ndl::lookup_ndl(client, &isbn_variants)
        .await
        .map_err(IsbnLookupError::Ndl)?
    {
        Some(mut book) => {
            super::ndl::authority::enrich(client, &mut book.authors).await;
            (book, "ndl")
        }
        None => {
            let Some(book) = super::open_library::lookup_isbn(client, &isbn_variants)
                .await
                .map_err(IsbnLookupError::OpenLibrary)?
            else {
                return Ok(None);
            };
            (book, "open_library")
        }
    };
    let expected_isbn13 = isbn_variants
        .iter()
        .find(|variant| variant.len() == 13)
        .map(String::as_str);
    let (cover_url, amazon_description, amazon_publish_date) =
        super::amazon::fetch_amazon_cover_with_retry(
            client,
            &isbn_variants,
            images_dir,
            isbn,
            expected_isbn13,
        )
        .await;
    let source = if source == "ndl" && cover_url.is_some() {
        "amazon"
    } else {
        source
    };
    let book = NewBook {
        isbn: Some(isbn.to_string()),
        isdn: None,
        jan: None,
        title: metadata.title,
        publisher: metadata.publisher,
        publish_date: amazon_publish_date
            .or_else(|| super::normalize_publish_date(metadata.publish_date.as_deref())),
        cover_url,
        description: amazon_description.or(metadata.description),
        title_transcription: metadata.title_transcription,
        series_title: metadata.series_title,
        series_title_transcription: metadata.series_title_transcription,
        alternative: metadata.alternative,
        alternative_transcription: metadata.alternative_transcription,
        volume: metadata.volume,
        volume_transcription: metadata.volume_transcription,
        price: metadata.price,
        extent: metadata.extent,
        jpno: metadata.jpno,
        ndl_url: metadata.ndl_url,
        authors: metadata.authors,
        media_type: None,
        catalog_number: None,
        artist: None,
        label: None,
        disc_count: None,
        tracks: None,
        isdn_region: None,
        isdn_class: None,
        isdn_type: None,
        isdn_rating_gender: None,
        isdn_rating_age: None,
        isdn_genre_code: None,
        isdn_genre_name: None,
        isdn_genre_user: None,
        isdn_c_code: None,
        isdn_author: None,
        isdn_shape: None,
        isdn_contents: None,
        isdn_barcode2: None,
        isdn_sample_image_url: None,
        isdn_useroption: None,
        isdn_external_links: None,
    };
    Ok(Some(IsbnLookup { book, source }))
}
