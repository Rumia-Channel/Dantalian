use crate::db::NewAuthor;

#[derive(Default)]
pub(crate) struct BookMetadata {
    pub(crate) title: String,
    pub(crate) publisher: Option<String>,
    pub(crate) publish_date: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) title_transcription: Option<String>,
    pub(crate) series_title: Option<String>,
    pub(crate) series_title_transcription: Option<String>,
    pub(crate) alternative: Option<String>,
    pub(crate) alternative_transcription: Option<String>,
    pub(crate) volume: Option<String>,
    pub(crate) volume_transcription: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) extent: Option<String>,
    pub(crate) jpno: Option<String>,
    pub(crate) ndl_url: Option<String>,
    pub(crate) authors: Vec<NewAuthor>,
}
