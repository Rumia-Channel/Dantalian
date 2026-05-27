mod amazon;
mod isdn;
mod musicbrainz;
pub(crate) mod ndl;

pub use amazon::lookup_isbn;
pub use isdn::lookup_isdn;
pub use musicbrainz::lookup_cd;
