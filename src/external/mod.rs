mod amazon;
mod discogs;
mod isdn;
mod musicbrainz;
pub(crate) mod ndl;

pub use amazon::lookup_isbn;
pub use discogs::lookup_cd_discogs;
pub use isdn::lookup_isdn;
pub use musicbrainz::lookup_cd;
