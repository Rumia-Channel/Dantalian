# 技術リファレンス（運用・API 契約・内部仕様）

Native / Worker 両ランタイムの運用仕様、ストレージ・アップロード設計、音声処理、API 更新契約、および回帰テスト手順をまとめています。

---

## 1. Native / Worker ランタイム比較

| 項目 | Native (`src/main.rs`) | Worker (`worker/src/lib.rs`) |
| --- | --- | --- |
| 実行基盤 | Rust (`axum` + `tokio`) | Cloudflare Workers (Wasm) |
| データベース | SQLite (`rusqlite`, スキーマ **19**) | Cloudflare D1 (`worker/migrations/0001`〜`0024`) |
| メディア保存 | ローカルディスク (`{DATA_DIR}/Dantalian/`) | Wasabi (S3 互換オブジェクトストレージ) |
| 認証 | 任意 (`DANTALIAN_API_TOKEN`)、非ループバック時は必須 | `DANTALIAN_API_TOKEN`（本番は Cloudflare Access 併用） |
| 書籍・大容量音声の送信 | サーバー受信（チャンクアップロード対応） | ブラウザから Wasabi へ presigned multipart 直送 |
| 音声省データ変換 | アプリ内バックグラウンドワーカー | Cloudflare Queue + Container (`audio_processor`) |
| バックアップ / S3 同期 | 組み込みスケジューラ | 対象外（D1 Time Travel / Wasabi を使用） |

### DB スキーマの整合性

- **Native**: 現行スキーマバージョンは **19**（[`src/db/schema.rs`](../src/db/schema.rs)）。対応版より新しい DB を開いた場合は起動を拒否します。
- **Worker**: `worker/migrations/` 配下の D1 マイグレーション（最新 `0024_restore_audio_file_names.sql`）を適用します。
- **整合性制約**: `grand_series_items` の参照整合性（`0016_grand_series_integrity.sql` および Native 側トリガ）により、存在しない書籍・CD・シリーズへの所属を拒否し、対象削除時に関連行を自動削除します。

### 読み取り一貫性とキャッシュ制御（Worker）

- D1 読み取りセッションは `first-primary` で開始します。
- Native / Worker の `/api/*` 応答とブラウザからの API 取得は `Cache-Control: no-store` とし、旧データ参照を防ぎます。
- 登録・編集後の別タブ更新やブラウザの「戻る（BFCache）」復元時は、`storage` / `visibilitychange` / `pageshow` イベントで一覧を自動再取得します（非表示タブは表示時まで遅延）。
- 静的アセット（HTML / CSS / JS / フォント）は `ASSET_VERSION` ハッシュによるキャッシュバスタ（`?v=...`）を使用します。Worker では `worker/public/_headers` でバージョン付きアセットに `immutable` キャッシュを付与します。

---

## 2. アップロード上限と送信経路

### Native

管理画面「設定 → アップロード」または DB の `settings` テーブルで上限を指定できます（最大 `4096` MB）。

| 設定キー | 既定値 | 対象 |
| --- | --- | --- |
| `upload.cover_max_mb` | `10` | カバー画像 |
| `upload.audio_max_mb` | `100` | 音声ファイル |
| `upload.file_max_mb` | `500` | 書籍ファイル（EPUB / PDF / ZIP） |

> リバースプロキシ（nginx の `client_max_body_size` 等）を前段に置く場合は、アプリ側設定以上の上限を設定してください。

### Worker（Presigned Multipart 直送）

Cloudflare Workers のリクエストボディ上限を回避するため、ブラウザから Wasabi へ直接 multipart アップロードを行います。

- **書籍ファイル（EPUB / ZIP / PDF）**: サイズを問わず**全件** presigned multipart で直送します（`POST /api/uploads/multipart/init` `object_kind: "epub"` → パート `PUT` → `complete` → `POST /api/books/:id/files/finalize`）。
  - ※ `object_kind: "epub"` と `epubs/` プレフィックスは EPUB / ZIP / PDF 共通の区分です。
- **音声ファイル**: **8 MiB 以上**（`PRESIGNED_MULTIPART_MIN_BYTES`）は presigned multipart で直送（`POST /api/{cds|books}/:id/tracks/:track_id/audio/multipart` → パート `PUT` → `complete` → `finalize`）。8 MiB 未満は Worker 経由で登録します。
- **カバー画像**: 上限 10 MiB として Worker 経由で登録します。
- **音声メタデータ解析**: Worker ランタイムではブラウザ内の Rust/WASM プリプロセッサ（`static/wasm/`、`scripts/build_audio_preprocessor.mjs`）で再生時間・技術タグをローカル解析してから送信します。
- **フォールバック禁止**: 直送経路やランタイム判定に失敗した場合でも、Worker 本文プロキシへフォールバックせず中断します。Native 用の上限設定（`upload.*_max_mb`）は Worker の設定画面には表示されません。

---

## 3. S3 メディア同期（Native / upload-only）

ローカルに保存された `images/` / `audio/` / `epubs/` を S3 互換ストレージへ **アップロード専用（削除・ミラーリングなし）** で同期します。

### S3 キー構成

`{DATA_DIR}/Dantalian/` 配下の各メディアディレクトリを再帰走査し、相対パスをそのまま S3 キーにマップします。`media_sync.s3_prefix`（または `MEDIA_SYNC_S3_PREFIX`）を `dantalian` とした場合の例:

- `{DATA_DIR}/Dantalian/images/abc.jpg` → `s3://{bucket}/dantalian/images/abc.jpg`
- `{DATA_DIR}/Dantalian/audio/encoded/opus/xyz.opus` → `s3://{bucket}/dantalian/audio/encoded/opus/xyz.opus`
- `{DATA_DIR}/Dantalian/epubs/book1.epub` → `s3://{bucket}/dantalian/epubs/book1.epub`

パストラバーサル防止のため、ルート外の相対パス成分（`..` や絶対パス等）は除外されます。`Content-Type` は拡張子から自動設定されます。

### 設定キー・環境変数

DB の `settings` テーブル（`media_sync.*`）または環境変数（`MEDIA_SYNC_*`）で設定します。S3 接続情報が空の場合はバックアップ設定（`backup.s3_*` / `BACKUP_S3_*`）を流用します。

| キー | 既定値 | 説明 |
| --- | --- | --- |
| `media_sync.enabled` | `false` | 同期ワーカーの有効化 |
| `media_sync.types` | `epubs,audio` | 同期対象（`images`, `audio`, `epubs` のカンマ区切り） |
| `media_sync.schedule_time` | `""` | 毎日の実行時刻（`HH:MM`）。空なら自動スケジュール無効 |
| `media_sync.schedule_tz` | `Asia/Tokyo` | スケジュールのタイムゾーン |
| `media_sync.s3_endpoint` | (backup) | S3 エンドポイント URL |
| `media_sync.s3_region` | (backup / `us-east-1`) | S3 リージョン |
| `media_sync.s3_bucket` | (backup) | バケット名 |
| `media_sync.s3_access_key` | (backup) | アクセスキー |
| `media_sync.s3_secret_key` | (backup) | シークレットキー |
| `media_sync.s3_prefix` | (backup / `""`) | キープレフィックス（先頭 `/`・`..` 不可） |

### 実行と API

- **手動実行**: `POST /api/media-sync/run`
  - バリデーション不備（`types` 空・認証情報不足・不正 prefix 等）は `400 Bad Request`、内部エラーは `500`、実行完了時は `200 OK`（`{ ok, scanned, uploaded, skipped, failed, missing_local }`、`failed > 0` の場合 `ok: false`）を返します。
- **スケジューラ**: 起動時から常駐し、待機中も最大 300 秒間隔で DB 設定の変化を監視してスケジュールを再計算します。アップロード前に `head_object` で存在確認し、NotFound 以外のエラー時は上書きせず失敗記録とします。

---

## 4. 音声再生・省データ再生・音量制御

### 音声アップロードのファイル名

- 音声の保存キー（`file_hash`）と表示名（`file_name`）は別です。Worker の直接送信・presigned multipart のどちらでも、表示・ダウンロードには元ファイル名を保持します。
- Worker migration `0024_restore_audio_file_names.sql` は、現在の音声オブジェクトと一致する `object_uploads.original_name` から、誤ってハッシュ名になった表示名だけを復元します。元名が未記録・曖昧な場合や、既に別名が設定されている場合は変更しません。

### トラック音声の並行アップロード

- 別トラックのアップロード開始で、先行トラックの時間取得・メタデータ保存を中断しません。同一トラックへの二重アップロードは完了まで拒否します。
- 時間欄が空なら自動保存し、既存値は確認してから上書きします。複数の確認は順に表示し、確認待ちの手入力は上書きしません。

### 省データ再生（Opus / AAC トランスコード）

管理画面の「省データ再生」を有効にすると、対象拡張子の音声をバックグラウンドで **Opus (Ogg)** と **AAC (ADTS)** へ変換します。

- **Native 保存先**: `{DATA_DIR}/Dantalian/audio/encoded/opus/{hash}.opus` および `.../aac/{hash}.aac`（メディア同期でも再利用）。
- **Worker 構成**: アップロード時・設定保存時・再生可否確認時に不足ジョブを Cloudflare Queue へ登録し、外部コンテナ（`container/Dockerfile` + `audio_processor`）が Wasabi から直接取得・変換・保存します。Worker 自体はデコード／エンコードを実行しません。
- **コーデック判定**: ブラウザの再生対応状況に応じて、`audio/ogg; codecs=opus` / `audio/aac`（ADTS 対応であり MP4 対応のみでは AAC 生成物対応とみなさない）を判定します。生成中は原音で再生し、次回ロード以降に変換済み音源を優先します。
- **旧形式ハッシュ互換**: 拡張子なしのハッシュや旧形式の `hash.flac` も同一の原音・ジョブ識別子へ正規化します。
- **オフライン音声キャッシュ**: ブラウザ側の音声キャッシュも現在の省データ設定に合わせて選択・置換します。

### 音量制御（iOS / iPadOS 対応）

- iOS / iPadOS Safari（デスクトップ表示モードを含む）では `HTMLMediaElement.volume` による音量変更が実出力に反映されないため、Web Audio API の `GainNode` を併用します。
- 再生操作の同一イベント内で `AudioContext` を再開し、署名付き URL リダイレクトに対応するため `crossOrigin="anonymous"` で取得します（Wasabi の `GET` 用 CORS 設定が必要）。

---

## 5. 書籍ファイルと API 更新契約

### 複数書籍ファイル

1 冊の書籍に対して複数の EPUB / PDF / 画像 ZIP を登録できます。

- 追加: `POST /api/books/{id}/files`（Worker では multipart 完了後に `/finalize`）
- 削除: `DELETE /api/books/{id}/files/{file_id}`（他書籍が同一ハッシュを参照している場合、実ファイルは削除しません）
- 一覧・詳細 API は `files` 配列（`id`, `book_id`, `file_hash`, `file_name`, `created_at`）を返します。

### 編集 API の共通契約

- **Partial update / null 消去**: 書籍・所蔵（`location` / `notes`）・借主（`notes`）・プレイリスト（`description` / `cover_cd_id`）・トラックメタデータの更新 API は、JSON で未指定のフィールドを保持し、明示的な `null` で値を消去します。
- **原子的更新**: 書籍と大シリーズ所属、トラックメタデータとアーティスト所属はそれぞれ一体で更新し、所属先エラー時にメタデータだけ先行更新しません。
- **シリーズ解除**: `PUT /api/books/{id}/series` で `series_id: null` を送るとシリーズ巻数も消去されます。
- **所属チェック**: CD / 書籍の所属先と一致しないトラック URL への操作は `404` を返します。
- **同時貸出制御**: 貸出中コピーへの再貸出は `409` を返します。Worker では事前 SELECT ではなく単一の条件付き INSERT により競合時も 1 件のみ成功させます。
- **大容量音声サイズ**: Worker の `raw_size_bytes` は符号付き 32 bit 上限（2 GiB）を超過しても正確に保持します。

### プレイリスト API

| メソッド・パス | 動作 |
| --- | --- |
| `POST /api/playlists` | `name` と任意の `description` / `cover_cd_id` / `track_ids` で作成 |
| `PUT /api/playlists/{id}` | 情報更新。`track_ids` 指定時のみ曲順置換（`[]` で空化） |
| `POST /api/playlists/{id}/tracks` | `{"track_ids":[1,2]}` または `{"track_id":1}` で末尾へ追加 |
| `DELETE /api/playlists/{id}/tracks` | `{"track_ids":[1,2]}` で一括削除し位置を詰める |

CD に属し `file_hash` を持つトラックのみ追加でき、重複 ID は先頭出現順を保って重複排除されます。

---

## 6. 回帰検証コマンド集

```sh
# Rust フォーマットチェック・単体テスト
cargo fmt --all -- --check
cargo check --no-default-features
cargo test --features native --all-targets
cargo test --manifest-path worker/Cargo.toml

# Worker アセット・契約ユニットテスト（外部ネットワーク不要）
node --test scripts/bibi/distribution.test.mjs
node --test worker/tests/data_freshness_unit.mjs
node --test worker/tests/player_volume_unit.mjs
node --test worker/tests/audio_source_selection_unit.mjs
node --test worker/tests/upload_target_unit.mjs
node --test worker/tests/upload_integration_unit.mjs

# Worker ビルド後のコントラクトテスト
node --test worker/tests/ndl_api_contract.mjs
node --test worker/tests/audio_data_saver_unit.mjs

# 起動済み Native / ローカル Worker に対する API 契約テスト
WORKER_BASE_URL=http://127.0.0.1:3000 node --test worker/tests/reliability_contract.mjs
READER_NAVIGATION_BACKEND=native WORKER_BASE_URL=http://127.0.0.1:3000 node --test worker/tests/reader_navigation_contract.mjs
```
