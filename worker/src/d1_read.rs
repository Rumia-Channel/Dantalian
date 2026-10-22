use worker::{D1Database, D1DatabaseSession, D1PreparedStatement, Request, Result};

/// A read-only D1 handle used by GET handlers.
///
/// Queries normally run through a D1 session constrained with
/// `first-unconstrained`, which lets Cloudflare route them to the nearest read
/// replica. Writes keep using the primary database. Writes also set a short
/// lived `dantalian_d1_primary` cookie (see `lib.rs`) so that reads immediately
/// after a mutation are served by the primary and observe the write.
pub(crate) enum ReadDb<'a> {
    Primary(&'a D1Database),
    Replica(D1DatabaseSession),
}

impl ReadDb<'_> {
    pub(crate) fn prepare<T: Into<String>>(&self, query: T) -> D1PreparedStatement {
        match self {
            Self::Primary(db) => db.prepare(query),
            Self::Replica(session) => session.prepare(query),
        }
    }
}

/// Build the read handle for a GET request.
///
/// Falls back to the primary database when the runtime does not support the D1
/// Sessions API (for example an older local miniflare), so read routing is never
/// a hard failure.
pub(crate) fn read_db<'a>(req: &Request, db: &'a D1Database) -> Result<ReadDb<'a>> {
    if prefers_primary(req) {
        return Ok(ReadDb::Primary(db));
    }
    match db.with_session(Some("first-unconstrained")) {
        Ok(session) => Ok(ReadDb::Replica(session)),
        Err(_) => Ok(ReadDb::Primary(db)),
    }
}

fn prefers_primary(req: &Request) -> bool {
    req.headers()
        .get("cookie")
        .ok()
        .flatten()
        .is_some_and(|cookies| {
            cookies
                .split(';')
                .any(|part| part.trim() == "dantalian_d1_primary=1")
        })
}
