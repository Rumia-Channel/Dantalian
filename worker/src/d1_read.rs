use worker::{D1Database, D1DatabaseSession, D1PreparedStatement, D1Result, Request, Result};

/// Minimal prepare surface shared by the primary database and a read-replica
/// session; repository read methods are generic over this so they can be used
/// from either path.
pub(crate) trait D1Prepare {
    fn prepare<T: Into<String>>(&self, query: T) -> D1PreparedStatement;
}

impl D1Prepare for D1Database {
    fn prepare<T: Into<String>>(&self, query: T) -> D1PreparedStatement {
        D1Database::prepare(self, query)
    }
}

impl D1Prepare for D1DatabaseSession {
    fn prepare<T: Into<String>>(&self, query: T) -> D1PreparedStatement {
        D1DatabaseSession::prepare(self, query)
    }
}

/// A read-only D1 handle used by GET handlers.
///
/// Each request starts at the latest primary version. A new unconstrained
/// session per GET has no bookmark from earlier writes and can move backwards
/// to an older replica. Subsequent reads within this request remain sequentially
/// consistent without an extra SQL statement or a time-limited cookie.
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

    pub(crate) async fn batch(
        &self,
        statements: Vec<D1PreparedStatement>,
    ) -> Result<Vec<D1Result>> {
        match self {
            Self::Primary(db) => db.batch(statements).await,
            Self::Replica(session) => session.batch(statements).await,
        }
    }
}

impl D1Prepare for ReadDb<'_> {
    fn prepare<T: Into<String>>(&self, query: T) -> D1PreparedStatement {
        ReadDb::prepare(self, query)
    }
}

/// Build the read handle for a GET request.
///
/// Falls back to the primary database when the runtime does not support the D1
/// Sessions API (for example an older local miniflare), so read routing is never
/// a hard failure.
pub(crate) fn read_db<'a>(_req: &Request, db: &'a D1Database) -> Result<ReadDb<'a>> {
    match db.with_session(Some("first-primary")) {
        Ok(session) => Ok(ReadDb::Replica(session)),
        Err(_) => Ok(ReadDb::Primary(db)),
    }
}
