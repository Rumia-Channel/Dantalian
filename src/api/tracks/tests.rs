use crate::{AppState, api};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use reqwest::Client;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Notify;
use tower::ServiceExt;

struct Fixture {
    app: Router,
    book_id: i64,
    book_track_id: i64,
    cd_id: i64,
    cd_track_id: i64,
}

fn fixture() -> Fixture {
    let db = crate::db::Db::new(":memory:").unwrap();
    let book = db
        .insert_book(&crate::db_models::NewBook {
            title: "Book".to_string(),
            ..Default::default()
        })
        .unwrap();
    let book_track = db
        .insert_track(
            book.id,
            &crate::db_models::NewTrack {
                disc_number: Some(1),
                track_number: 3,
                title: "Book track".to_string(),
                duration: Some("03:00".to_string()),
            },
        )
        .unwrap();
    let cd = db
        .insert_cd(&crate::db_models::NewCd {
            title: "Album".to_string(),
            ..Default::default()
        })
        .unwrap();
    let cd_track = db
        .insert_track_for_cd(
            cd.id,
            &crate::db_models::NewTrack {
                disc_number: Some(1),
                track_number: 3,
                title: "CD track".to_string(),
                duration: Some("03:00".to_string()),
            },
        )
        .unwrap();
    db.update_track_audio(book_track.id, Some("book-hash"), Some("book.mp3"))
        .unwrap();
    db.update_track_audio(cd_track.id, Some("cd-hash"), Some("cd.mp3"))
        .unwrap();

    let state = AppState {
        db,
        client: Client::new(),
        client_ipv4: Client::new(),
        images_dir: Arc::new(String::new()),
        object_storage: Arc::new(
            crate::adapters::native_object_storage::NativeObjectStorage::new(".", ""),
        ),
        audio_dir: Arc::new(String::new()),
        audio_encoding_notify: Arc::new(Notify::new()),
        epubs_dir: Arc::new(String::new()),
        uploads_dir: Arc::new(String::new()),
        discogs_token: String::new(),
        musicbrainz_contact: String::new(),
        auth: crate::auth::AuthConfig::default(),
    };
    Fixture {
        app: Router::new().nest("/api", api::routes()).with_state(state),
        book_id: book.id,
        book_track_id: book_track.id,
        cd_id: cd.id,
        cd_track_id: cd_track.id,
    }
}

async fn put(app: &Router, uri: &str, body: Value) -> StatusCode {
    let request = Request::builder()
        .method("PUT")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

async fn track(app: &Router, uri: &str, id: i64) -> Value {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let tracks: Vec<Value> = serde_json::from_slice(&body).unwrap();
    tracks
        .into_iter()
        .find(|track| track["id"].as_i64() == Some(id))
        .expect("track")
}

#[tokio::test]
async fn position_updates_keep_the_other_position_and_metadata() {
    let fixture = fixture();

    assert_eq!(
        put(
            &fixture.app,
            &format!(
                "/api/books/{}/tracks/{}",
                fixture.book_id, fixture.book_track_id
            ),
            json!({"disc_number": 2}),
        )
        .await,
        StatusCode::OK
    );
    let stored = track(
        &fixture.app,
        &format!("/api/books/{}/tracks", fixture.book_id),
        fixture.book_track_id,
    )
    .await;
    assert_eq!(stored["disc_number"], 2);
    assert_eq!(stored["track_number"], 3);
    assert_eq!(stored["title"], "Book track");
    assert_eq!(stored["duration"], "03:00");
    assert_eq!(stored["file_hash"], "book-hash");

    assert_eq!(
        put(
            &fixture.app,
            &format!(
                "/api/books/{}/tracks/{}",
                fixture.book_id, fixture.book_track_id
            ),
            json!({"track_number": 7}),
        )
        .await,
        StatusCode::OK
    );
    let stored = track(
        &fixture.app,
        &format!("/api/books/{}/tracks", fixture.book_id),
        fixture.book_track_id,
    )
    .await;
    assert_eq!(stored["disc_number"], 2);
    assert_eq!(stored["track_number"], 7);
    assert_eq!(stored["title"], "Book track");
    assert_eq!(stored["file_hash"], "book-hash");
}

#[tokio::test]
async fn cd_position_updates_keep_the_other_position_and_metadata() {
    let fixture = fixture();

    assert_eq!(
        put(
            &fixture.app,
            &format!("/api/cds/{}/tracks/{}", fixture.cd_id, fixture.cd_track_id),
            json!({"disc_number": 2}),
        )
        .await,
        StatusCode::OK
    );
    let stored = track(
        &fixture.app,
        &format!("/api/cds/{}/tracks", fixture.cd_id),
        fixture.cd_track_id,
    )
    .await;
    assert_eq!(stored["disc_number"], 2);
    assert_eq!(stored["track_number"], 3);
    assert_eq!(stored["title"], "CD track");
    assert_eq!(stored["file_hash"], "cd-hash");

    assert_eq!(
        put(
            &fixture.app,
            &format!("/api/cds/{}/tracks/{}", fixture.cd_id, fixture.cd_track_id),
            json!({"track_number": 7}),
        )
        .await,
        StatusCode::OK
    );
    let stored = track(
        &fixture.app,
        &format!("/api/cds/{}/tracks", fixture.cd_id),
        fixture.cd_track_id,
    )
    .await;
    assert_eq!(stored["disc_number"], 2);
    assert_eq!(stored["track_number"], 7);
    assert_eq!(stored["file_hash"], "cd-hash");
}

#[tokio::test]
async fn invalid_positions_are_rejected_without_writing() {
    let fixture = fixture();
    let book_uri = format!(
        "/api/books/{}/tracks/{}",
        fixture.book_id, fixture.book_track_id
    );
    let cd_uri = format!("/api/cds/{}/tracks/{}", fixture.cd_id, fixture.cd_track_id);

    for body in [
        json!({"disc_number": 0}),
        json!({"disc_number": -1}),
        json!({"track_number": 0}),
        json!({"track_number": 2_147_483_648_i64}),
        json!({"disc_number": "2"}),
    ] {
        assert_eq!(
            put(&fixture.app, &book_uri, body.clone()).await,
            StatusCode::BAD_REQUEST,
            "{body} should be rejected"
        );
        assert_eq!(
            put(&fixture.app, &cd_uri, body.clone()).await,
            StatusCode::BAD_REQUEST,
            "{body} should be rejected"
        );
    }

    // A rejected request must not write the valid field of the same body.
    assert_eq!(
        put(
            &fixture.app,
            &book_uri,
            json!({"disc_number": 5, "track_number": 0}),
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    let stored = track(
        &fixture.app,
        &format!("/api/books/{}/tracks", fixture.book_id),
        fixture.book_track_id,
    )
    .await;
    assert_eq!(stored["disc_number"], 1);
    assert_eq!(stored["track_number"], 3);
}

#[tokio::test]
async fn track_updates_reject_tracks_of_another_parent() {
    let fixture = fixture();

    assert_eq!(
        put(
            &fixture.app,
            &format!(
                "/api/books/{}/tracks/{}",
                fixture.book_id, fixture.cd_track_id
            ),
            json!({"disc_number": 2}),
        )
        .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        put(
            &fixture.app,
            &format!(
                "/api/cds/{}/tracks/{}",
                fixture.cd_id, fixture.book_track_id
            ),
            json!({"disc_number": 2}),
        )
        .await,
        StatusCode::NOT_FOUND
    );

    let stored = track(
        &fixture.app,
        &format!("/api/cds/{}/tracks", fixture.cd_id),
        fixture.cd_track_id,
    )
    .await;
    assert_eq!(stored["disc_number"], 1);
}
