mod amazon_api;
mod audio_api;
mod audio_job_api;
mod audio_job_repository;
mod auth;
mod author_api;
mod author_repository;
mod book_api;
mod borrower_api;
mod borrower_repository;
mod cd_api;
mod copy_api;
mod cover_api;
mod error;
mod external_api;
mod grand_series_api;
mod label_api;
mod label_repository;
mod media_sync_api;
mod multipart_api;
mod musicbrainz_api;
mod object_api;
mod playlist_api;
mod series_api;
mod series_repository;
mod settings_api;
mod storage_location_api;
mod storage_location_repository;
mod track_api;
mod wasabi;
pub mod wasabi_config;

use worker::*;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let auth_response = if req.path().starts_with("/api/internal/audio/jobs/") {
        auth::authorize_processor(&env, &req)?
    } else {
        auth::authorize(&env, &req)?
    };
    if let Some(response) = auth_response {
        return Ok(response);
    }
    Router::new()
        .get_async("/api/health", |_req, ctx| async move {
            Response::from_json(&serde_json::json!({
                "ok": true,
                "runtime": "cloudflare-worker",
                "authentication_required": auth::authentication_required(&ctx.env),
            }))
        })
        .get_async("/api/series", series_api::list)
        .get_async("/api/books/:book_id/copies", copy_api::list)
        .post_async("/api/books/:book_id/copies", copy_api::create)
        .put_async("/api/copies/:id", copy_api::update)
        .delete_async("/api/copies/:id", copy_api::delete)
        .post_async("/api/copies/:id/lend", copy_api::lend)
        .post_async("/api/copies/:id/return", copy_api::return_copy)
        .get_async("/api/settings", settings_api::get)
        .put_async("/api/settings", settings_api::update)
        .get_async("/api/copies/:id/history", copy_api::history)
        .post_async("/api/series", series_api::create)
        .get_async("/api/grand-series", grand_series_api::list)
        .post_async("/api/grand-series", grand_series_api::create)
        .put_async("/api/grand-series/:id", grand_series_api::rename)
        .delete_async("/api/grand-series/:id", grand_series_api::delete)
        .post_async("/api/grand-series/:id/items", grand_series_api::add_item)
        .delete_async(
            "/api/grand-series/:id/items/:item_type/:item_id",
            grand_series_api::remove_item,
        )
        .put_async("/api/series/:id", series_api::rename)
        .get_async("/api/books", book_api::list)
        .get_async("/api/books/:id", book_api::get)
        .post_async("/api/books", book_api::register_book)
        .post_async("/api/books/isdn", book_api::register_isdn)
        .post_async("/api/books/manual", book_api::register_manual)
        .put_async("/api/books/:id", book_api::update)
        .delete_async("/api/books/:id", book_api::delete)
        .put_async("/api/books/:id/series", book_api::set_series)
        .post_async("/api/books/:id/authors/:author_id", book_api::author_add)
        .delete_async("/api/books/:id/authors/:author_id", book_api::author_remove)
        .put_async("/api/books/:id/authors/:author_id", book_api::author_order)
        .post_async("/api/books/:id/cover", object_api::book_cover)
        .delete_async("/api/books/:id/cover", object_api::delete_book_cover)
        .post_async("/api/books/:id/epub", object_api::book_epub)
        .delete_async("/api/books/:id/epub", object_api::delete_book_epub)
        .get_async("/api/audio/stream/:file_hash", object_api::stream)
        .get_async("/audio/:file_hash", object_api::stream)
        .get_async("/images/:file_hash", object_api::image)
        .get_async("/epubs/:file_hash", object_api::epub)
        .post_async("/api/audio/encode/:format", audio_api::encode)
        .post_async("/api/audio/jobs", audio_job_api::create)
        .get_async("/api/audio/jobs/:id", audio_job_api::get)
        .post_async("/api/audio/jobs/:id/retry", audio_job_api::retry)
        .post_async("/api/audio/jobs/claim", audio_job_api::claim)
        .post_async("/api/audio/jobs/renew", audio_job_api::renew)
        .post_async("/api/audio/jobs/complete", audio_job_api::complete)
        .post_async("/api/audio/jobs/fail", audio_job_api::fail)
        .post_async(
            "/api/internal/audio/jobs/:id/claim",
            audio_job_api::claim_by_id,
        )
        .post_async("/api/internal/audio/jobs/:id/renew", audio_job_api::renew)
        .post_async(
            "/api/internal/audio/jobs/:id/complete",
            audio_job_api::complete,
        )
        .post_async("/api/internal/audio/jobs/:id/fail", audio_job_api::fail)
        .post_async("/api/uploads/covers/init", cover_api::init)
        .post_async("/api/uploads/covers/complete", cover_api::complete)
        .post_async("/api/uploads/multipart/init", multipart_api::init)
        .post_async(
            "/api/uploads/multipart/:id/parts/:part_number/sign",
            multipart_api::sign_part,
        )
        .post_async(
            "/api/uploads/multipart/:id/complete",
            multipart_api::complete,
        )
        .delete_async("/api/uploads/multipart/:id", multipart_api::abort)
        .get_async("/api/authors", author_api::list)
        .post_async("/api/authors", author_api::create)
        .get_async("/api/authors/:id", author_api::get)
        .put_async("/api/authors/:id", author_api::update)
        .delete_async("/api/authors/:id", author_api::delete)
        .delete_async("/api/series/:id", series_api::delete)
        .get_async("/api/labels", label_api::list)
        .post_async("/api/labels", label_api::create)
        .put_async("/api/labels/:id", label_api::rename)
        .get_async("/api/borrowers", borrower_api::list)
        .post_async("/api/borrowers", borrower_api::create)
        .post_async("/api/cds/:id/cover", object_api::cd_cover)
        .delete_async("/api/cds/:id/cover", object_api::delete_cd_cover)
        .post_async(
            "/api/books/:id/tracks/:track_id/audio",
            object_api::book_audio,
        )
        .delete_async(
            "/api/books/:id/tracks/:track_id/audio",
            object_api::delete_book_audio,
        )
        .post_async("/api/cds/:id/tracks/:track_id/audio", object_api::cd_audio)
        .delete_async(
            "/api/cds/:id/tracks/:track_id/audio",
            object_api::delete_cd_audio,
        )
        .put_async("/api/borrowers/:id", borrower_api::update)
        .delete_async("/api/borrowers/:id", borrower_api::delete)
        .delete_async("/api/labels/:id", label_api::delete)
        .get_async("/api/storage-locations", storage_location_api::list)
        .post_async("/api/storage-locations", storage_location_api::create)
        .put_async("/api/storage-locations/:id", storage_location_api::update)
        .delete_async("/api/storage-locations/:id", storage_location_api::delete)
        .get_async("/api/cds", cd_api::list)
        .post_async("/api/cds", cd_api::create)
        .put_async("/api/cds/:id", cd_api::update)
        .delete_async("/api/cds/:id", cd_api::delete)
        .get_async("/api/cds/:id/tracks", track_api::list_cd)
        .post_async("/api/cds/:id/tracks", track_api::add_cd)
        .put_async("/api/cds/:id/tracks/:track_id", track_api::update_cd)
        .delete_async("/api/cds/:id/tracks/:track_id", track_api::delete_cd)
        .get_async("/api/books/:id/tracks", track_api::list_book)
        .put_async("/api/books/:id/tracks/:track_id", track_api::update_book)
        .delete_async("/api/books/:id/tracks/:track_id", track_api::delete_book)
        .post_async("/api/media-sync/run", media_sync_api::run)
        .get_async(
            "/api/books/:id/tracks/:track_id/metadata",
            track_api::get_book_metadata,
        )
        .put_async(
            "/api/books/:id/tracks/:track_id/metadata",
            track_api::put_book_metadata,
        )
        .get_async(
            "/api/cds/:id/tracks/:track_id/metadata",
            track_api::get_cd_track_metadata,
        )
        .put_async(
            "/api/cds/:id/tracks/:track_id/metadata",
            track_api::put_cd_track_metadata,
        )
        .get_async("/api/cds/:id/metadata", track_api::get_cd_metadata)
        .put_async("/api/cds/:id/metadata", track_api::put_cd_metadata)
        .get_async("/api/cds/:id/album-tags", track_api::album_tags)
        .get_async("/api/track-metadata/search", track_api::search)
        .post_async("/api/cds/:id/authors/:author_id", cd_api::add_author)
        .delete_async("/api/cds/:id/authors/:author_id", cd_api::remove_author)
        .put_async(
            "/api/cds/:id/authors/:author_id",
            cd_api::update_author_order,
        )
        .post_async(
            "/api/cds/:id/authors/from-names",
            cd_api::add_authors_from_names,
        )
        .get_async("/api/playlists", playlist_api::list)
        .post_async("/api/playlists", playlist_api::create)
        .get_async("/api/playlists/:id", playlist_api::get)
        .put_async("/api/playlists/:id", playlist_api::update)
        .delete_async("/api/playlists/:id", playlist_api::delete)
        .put_async("/api/playlists/:id/tracks", playlist_api::set_tracks)
        .post_async("/api/playlists/:id/tracks", playlist_api::add_track)
        .delete_async(
            "/api/playlists/:id/tracks/:track_id",
            playlist_api::remove_track,
        )
        .run(req, env)
        .await
}

// The Worker has no local filesystem or SQLite backup API; D1 backups stay
// in Cloudflare's managed backup/export boundary.
#[event(scheduled)]
pub async fn scheduled(event: ScheduledEvent, env: Env, ctx: ScheduleContext) {
    if let Err(error) = audio_job_api::recover_and_dispatch(&env).await {
        worker::console_error!("scheduled audio dispatch recovery failed: {error}");
    }
    if let Err(error) = audio_job_api::enqueue_data_saver_jobs(&env).await {
        worker::console_error!("scheduled data saver job registration failed: {error}");
    }
    if let Err(error) = media_sync_api::run_scheduled(event, env, ctx).await {
        worker::console_error!("scheduled media sync failed: {error}");
    }
}
