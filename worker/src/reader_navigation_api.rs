use dantalian::application::reader_navigation::{
    self, NAVIGATION_QUERY, NavigationContext, NavigationError, NavigationQuery, NavigationRow,
};
use worker::{D1Type, Request, Response, Result, RouteContext};

fn response(result: std::result::Result<NavigationContext, NavigationError>) -> Result<Response> {
    let mut response = match result {
        Ok(context) => Response::from_json(&context)?,
        Err(error) => Response::from_json(&error.body())?.with_status(error.status()),
    };
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let mut query = NavigationQuery {
        book_key: None,
        content_hash: None,
        return_to: None,
    };
    for (key, value) in url.query_pairs() {
        let slot = match key.as_ref() {
            "book_key" => &mut query.book_key,
            "content_hash" => &mut query.content_hash,
            "return_to" => &mut query.return_to,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return response(Err(NavigationError::InvalidRequest));
        }
    }
    let request = match query.validate() {
        Ok(request) => request,
        Err(error) => return response(Err(error)),
    };
    let db = ctx.env.d1("DB")?;
    let db = crate::d1_read::read_db(&req, &db)?;
    let large_id;
    let id = match i32::try_from(request.id) {
        Ok(id) => D1Type::Integer(id),
        Err(_) => {
            large_id = request.id.to_string();
            D1Type::Text(&large_id)
        }
    };
    let statement = match db
        .prepare(NAVIGATION_QUERY)
        .bind_refs([&id, &D1Type::Text(&request.content_hash)])
    {
        Ok(statement) => statement,
        Err(_) => return response(Err(NavigationError::Database)),
    };
    match statement.first::<NavigationRow>(None).await {
        Ok(row) => response(reader_navigation::resolve(row, request)),
        Err(_) => response(Err(NavigationError::Database)),
    }
}
