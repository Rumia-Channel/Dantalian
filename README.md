# Dantalian

## FDK AAC ライセンス・特許告知

**2026-08-31:** 本プロジェクトは、`fdk-aac-rust` を通じて
**Third-Party Modified Version of the Fraunhofer FDK AAC Codec Library for Android**
を含みます。これは Fraunhofer の公式プロジェクトではなく、Fraunhofer の承認・推奨を受けていません。
`fdk-aac-rust` は Fraunhofer FDK AAC の純 Rust 移植版であり、C/C++ コンパイラは不要です。
Rust ポートへの変更とこの告知の日付を明示しています。

完全なライセンス本文は [`NOTICE`](NOTICE) に保持しています。
`MODULE_LICENSE_FRAUNHOFER` も配布物に含めてください。NOTICE は特許ライセンスを
付与しないため、AAC のエンコード・デコードを配布または利用する場合は、対象となる
特許権者またはライセンス管理者から必要な許諾を取得する責任があります。
Fraunhofer の名称を承認・推奨の表示に使用してはなりません。

配布時の確認事項は [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md) にまとめています。

## 外部 API / スクレイピング

### 国立国会図書館 NDL SRU API（書籍メタデータ取得）

- **エンドポイント**: `https://ndlsearch.ndl.go.jp/api/sru?operation=searchRetrieve&version=1.2&recordSchema=dcndl&onlyBib=true&maximumRecords=1&startRecord=1&recordPacking=xml&query=isbn%3D%22{isbn}%22`
- **用途**: ISBN をキーにして書籍のメタデータを取得する
- **形式**: SRU/XML（dcndl RDF/XML スキーマ）
- **取得フィールド**: タイトル（`dcterms:title`）、著者（`dc:creator`）、出版社（`dcterms:publisher` > `foaf:Agent` > `foaf:name`）、出版日（`dcterms:date`）、解説（`dcterms:description`、メタデータノイズ除外）、タイトル読み（`dc:title` > `rdf:Description` > `dcndl:transcription`）、著者読み（`dcterms:creator` > `foaf:Agent` > `dcndl:transcription`）、シリーズタイトル（`dcndl:seriesTitle` > `rdf:Description` > `rdf:value`）、価格（`dcndl:price`）、ページ数（`dcterms:extent`）、NDL URL（`dcndl:BibResource` の `rdf:about` 属性）
- **実装**: `quick-xml` でパース。`rdf:about` 属性に `#material` を含む最初の `dcndl:BibResource` 要素を対象とし、要素パスの深さとタグ名の組み合わせで各フィールドを識別

### Amazon.co.jp スクレイピング（表紙画像取得）

表紙画像は NDL には含まれないため、Amazon.co.jp からスクレイピングで取得する。`scraper` クレートで HTML をパースし、`tokio::task::spawn_blocking` 内で実行（`scraper::Html` は `Send` でないため）。

#### 1. 検索ページ（商品リンク取得）

- **URL**: `https://www.amazon.co.jp/s?k={isbn}`
- **セレクタ**: `[cel_widget_id^="MAIN-SEARCH_RESULTS-"]`（接頭辞マッチ）
- **処理**: ウィジェット内の商品ページ候補を収集し、Kindle / Audible / 電子書籍リンクを除外する。紙版候補を優先して商品詳細ページを取得する。
- 検索結果の商品詳細ページで ISBN-13 を取得できない場合でも、詳細ページの形式選択 (`tmm-grid-swatch-*`) でハードカバー等の紙版が選択されていれば、紙版の商品情報・表紙候補として扱う。

#### 2. 年齢確認（ブラックカーテン）バイパス

- **検出**: 商品詳細ページ HTML に `black-curtain-verification` が含まれている場合
- **対応**: `GET https://www.amazon.co.jp/black-curtain/save-eligibility/black-curtain?returnUrl={/dp/パス}` にアクセスし、サーバーサイドで `session-id` クッキーを更新してから商品ページを再取得
- **要件**: `reqwest` クライアントに `cookie_store(true)` を設定（`cookies` feature 有効化）

#### 3. 商品詳細ページ（表紙画像 URL 抽出）

- **URL**: 検索ページから取得した商品ページ URL
- **抽出順序**（優先度順）:
  1. `img#landingImage` の `data-old-hires` 属性（高解像度画像 URL）
  2. `img#landingImage` の `data-a-dynamic-image` 属性（JSON 形式、キーの文字列長が最長のものを選択）
  3. `img#imgBlkFront` の `src` 属性（低解像度フォールバック）

#### 4. 表紙画像ダウンロード

- 画像をローカルの `images/` ディレクトリに保存
- ファイル名: SHA3-256 ハッシュ（URL-safe Base64 エンコード）+ 拡張子（Content-Type から判定: jpg/png/webp/gif）
- 既に同名ファイルが存在する場合はダウンロードをスキップ
- User-Agent: `ua_generator::ua::spoof_ua()` でランダム生成（`amazon_request` ヘルパー経由）

## 機能

- ISBN による書籍登録（NDL メタデータ + Amazon 表紙画像）
- ユーザー定義シリーズ管理（作成・変更・削除・書籍への割り当て）
- 本だけを扱う一覧（`/books/`）。著者別／一覧の切り替え、検索、並び順、シリーズ・大シリーズの絞り込みに対応
- レスポンシブ対応ダークテーマ UI
- コンテンツ幅設定（LocalStorage に保存）

## 環境変数

| 変数 | 説明 | デフォルト |
|------|------|------------|
| `DATA_DIR` | データ保存ディレクトリ | `~/Documents` (Windows: `C:\Users\{user}\Documents`) |
| `PORT` | サーバー待受ポート | `3000` |
| `RUST_LOG` | ログレベル | `dantalian=debug` でデバッグ出力 |

## データ構成

```
{DATA_DIR}/
  Dantalian/
    db/
      dantalian.db    # SQLite データベース
    images/       # 表紙画像ファイル (jpg, jpeg, png, webp, gif)
    audio/        # 音声ファイル (mp3, wav, flac, ogg, m4a, aac, opus, webm)
    epubs/        # 書籍ファイル (epub, pdf, zip)
```

## メディア同期 (S3 upload-only)

ローカルに保存されたメディアファイル (images / audio / epubs) を **upload-only** で S3 互換ストレージに同期する機能です。
S3 側からの削除やミラーリングは行いません (ローカルに存在しないオブジェクトは S3 上でも削除されません)。

### S3 キー構成

ローカル相対パスがそのまま S3 キーに対応します。**再帰的に**すべてのサブディレクトリを走査し、各ファイルの root からの相対パスを S3 キーとして使用します。`MEDIA_SYNC_S3_PREFIX` (または `BACKUP_S3_PREFIX`) を `dantalian` にすると次のようになります:

- `{DATA_DIR}/images/abc.jpg`             → `s3://{bucket}/dantalian/images/abc.jpg`
- `{DATA_DIR}/audio/foo/bar.mp3`          → `s3://{bucket}/dantalian/audio/foo/bar.mp3`
- `{DATA_DIR}/epubs/series1/book1.epub`   → `s3://{bucket}/dantalian/epubs/series1/book1.epub`

パス・トラバーサル防止のため、ルート外のコンポーネント (`..`、`\` 絶対パス、Windows ドライブプレフィックス等) を含む相対パスはアップロード対象外となります。

`media_sync.s3_prefix` は通常の S3 プレフィックスとして扱われます。`..` セグメントや先頭の `/` を含む値はバリデーションエラーになります (末尾の `/` はトリムされます)。

### 設定キー

DB の `settings` テーブル (`media_sync.*`) または環境変数 (`MEDIA_SYNC_*`) で指定します。S3 関連キーが空の場合はバックアップ設定 (`backup.s3_*` / `BACKUP_S3_*`) を流用します。

| キー | デフォルト | 説明 |
|------|-----------|------|
| `media_sync.enabled` | `false` | 同期の有効化 |
| `media_sync.types` | `epubs,audio` | 同期対象 (`images`, `audio`, `epubs` をカンマ区切り) |
| `media_sync.schedule_time` | `""` | 毎日同時刻に同期 (HH:MM)。空ならスケジュール無効 |
| `media_sync.schedule_tz` | `Asia/Tokyo` | スケジュール時刻のタイムゾーン |
| `media_sync.s3_endpoint` | (backup) | S3 エンドポイント URL |
| `media_sync.s3_region` | (backup / `us-east-1`) | S3 リージョン |
| `media_sync.s3_bucket` | (backup) | バケット名 |
| `media_sync.s3_access_key` | (backup) | アクセスキー |
| `media_sync.s3_secret_key` | (backup) | シークレットキー |
| `media_sync.s3_prefix` | (backup / `""`) | キープレフィックス (末尾 `/` は不要) |

### アップロード上限

DB の `settings` テーブル (管理画面「設定」→「アップロード上限」からも変更可能) で指定します。単位は MB。

| キー | デフォルト | 説明 |
|------|-----------|------|
| `upload.cover_max_mb` | `10` | カバー画像の上限 |
| `upload.audio_max_mb` | `100` | 音声ファイルの上限 |
| `upload.file_max_mb` | `500` | 書籍ファイル (epub/pdf/zip) の上限 |

上限は 4096MB (4GB) まで。これは**アプリ側**の上限です。前面のリバースプロキシ (nginx: `client_max_body_size` 等) や Cloudflare の上限も、これ以上に引き上げる必要があります (実効上限は両者の小さい方)。

### 省データ再生

管理画面の「省データ再生」を有効にすると、指定した拡張子の音声を初回再生時に Opus と AAC へ変換します。生成物は次の場所へ保存され、以後の再生とメディア同期で再利用されます。

```
{DATA_DIR}/audio/encoded/opus/{hash}.opus
{DATA_DIR}/audio/encoded/aac/{hash}.aac
```

変換はサーバー側で行われ、再生時に生成済みのファイルを再利用します。

ブラウザのオフラインキャッシュも現在の設定に合わせます。有効時は対象拡張子の Opus/AAC、無効時は原音だけを再生・再利用します。設定切替で形式が合わなくなったキャッシュは未取得扱いになりますが、状態確認だけでは削除せず、新しい音声の取得・保存が成功してから置き換えます。

### API

- `POST /api/media-sync/run` — 手動実行。レスポンスは `ok` / `scanned` / `uploaded` / `skipped` / `failed` / `missing_local` を含む summary JSON。
 - 設定不足 (types 空 / S3 認証情報欠如 / 不正な prefix 等) は **400 BAD_REQUEST** `{ok:false, error:...}`。
 - 内部エラー (AWS SDK 関連等) は **500 INTERNAL_SERVER_ERROR** `{ok:false, error:...}`。
 - 同期完了 (`failed > 0` を含む) は **200 OK** で summary JSON。`ok` フィールドは `failed > 0` のとき `false` になります。

### 動作

- 起動時にバックグラウンドのスケジューラ worker を**常時**起動します。`media_sync.enabled` は worker 内側で毎ループ確認されるため、無効化すれば worker は短時間 idle スリープに戻ります。
- 有効時は `schedule_time` と `schedule_tz` から次の予定時刻を算出し、そこまで最大 **300 秒単位のチャンク**で sleep します。待機中も DB を再読込し、`media_sync.*` のいずれかが変化したら同期を実行せずループ先頭に戻って再計算します (設定変更が5分以内に反映されます)。
- 設定バリデーション (`config.validate()`) に失敗した状態では同期を実行せず、warn ログを出して短時間スリープしてから再確認します。`types` が空や S3 認証情報が不足したまま成功扱いにはなりません。
- 同期は images / audio / epubs ディレクトリを**再帰的に**走査し、拡張子でフィルタしたうえで `head_object` で既存確認 → なければ `put_object` します。`head_object` が NotFound 以外のエラー (401/403/5xx/タイムアウト等) を返した場合は上書きせず失敗として記録します。
- Content-Type は拡張子から自動設定します。

## 編集 API の更新契約

Native / Worker の書籍の nullable フィールド、所蔵の `location` / `notes`、借主の `notes`、プレイリストの `description` / `cover_cd_id`、トラックメタデータは、JSON の未指定フィールドを保持し、明示的な `null` で値を消去します。

- 書籍と大シリーズ所属、トラックメタデータとアーティスト所属はそれぞれまとめて更新します。不正な所属先で失敗した場合、先に情報だけが書き換わることはありません。
- 新規トラックでも `artists` のみのメタデータ更新を保存・再取得できます。Worker の `raw_size_bytes` は符号付き 32 bit の上限では制限せず、対応する大容量音声のバイト数を保持します。
- `PUT /api/books/{id}/series` で `series_id: null` を指定すると、シリーズ巻数も消去します。
- 所属先の異なる URL からのトラック更新・メタデータ取得・削除は `404` を返します。
- 貸出中の所蔵への再貸出は `409` を返します。同時に貸し出しても成功するのは一件だけです。Worker は事前 SELECT ではなく単一の条件付き INSERT で貸出可否を判定するため、同時要求から複数の貸出中レコードを作りません。返却後の再貸出は可能です。所蔵一覧の `lent_to` には借主名を返します。

### プレイリスト

| メソッド・パス | リクエスト・動作 |
|---|---|
| `POST /api/playlists` | `name` と任意の `description` / `cover_cd_id` / `track_ids`。初期トラックを含めて作成します。 |
| `PUT /api/playlists/{id}` | 情報更新。`track_ids` を指定した場合だけ曲順を置き換え、`[]` で空にします。 |
| `POST /api/playlists/{id}/tracks` | `{"track_ids":[1,2]}` でサーバー側の現在の末尾へ追加します。単曲の `{"track_id":1}` も受け付けますが、両方の同時指定はできません。 |
| `DELETE /api/playlists/{id}/tracks` | `{"track_ids":[1,2]}` でまとめて削除し、残った位置を詰めます。 |

追加できるのは CD に属し `file_hash` を持つトラックだけです。重複した ID は先頭の出現順を保って一つにまとめます。作成・曲順置換・一括追加・一括削除は原子的に実行します。

Native / Worker ともに取得・更新レスポンスの `tracks` は `{ position, track, cd }` の配列です。追加・削除時はクライアントの古い一覧を使った曲順置換ではなく、上記の一括操作を使用してください。

## DB スキーマの整合性

Native の現行スキーマは `13` です。対応版より新しい DB を開いた場合、テーブルを削除せず起動を拒否します。新しいバージョンで更新した DB を古い実行ファイルで開かないでください。

Worker の更新時は `worker/migrations/0016_grand_series_integrity.sql` を D1 に適用してください。この移行は存在しない書籍・CD・シリーズへの大シリーズ所属を除去し、以後の不正な所属を拒否します。対象の削除時には所属も自動的に除去します。Native にも同じ整合性制約を適用します。

## 更新経路の回帰検証

単体テスト:

```sh
cargo test --features native --all-targets
cargo test --manifest-path worker/Cargo.toml
```

API の検証には使い捨て DB の Native またはローカル Worker を起動し、`WORKER_BASE_URL` にその URL を指定して次を実行します。認証を有効にした場合は `WORKER_API_TOKEN` も指定してください。

```sh
node --test worker/tests/reliability_contract.mjs
```

この API 回帰検証は Worker CI でも実行します。新規トラックのアーティストのみの更新と、2 GiB 以上の音声サイズを含むメタデータ更新も検証対象です。

## 本一覧 (`/books/`)

ヘッダーの「本」から開く、本だけの一覧です。`media_type` が `cd` または `audiobook` の項目を除いた本を表示します。

- 表示と操作は一覧 (`/`) と同じ `static/js/book-grid.js` を共有し、`<body data-catalogue="books">` で分類を切り替えます。設定バーは `static/js/catalogue-controls.js` が両ページ分を組み立てます。
- 並び順や表示形式の保存キーは `dantalian_books_*` に分離し、一覧ページの設定 (`dantalian_*` / `dantalian_type_filter`) とは独立して保存します。
- 種別フィルタ（全／書籍／CD／AB）は本ページには表示しません。検索対象は本ページで表示中の項目だけです。
- シリーズ／大シリーズのカードとモーダルは、本ページで表示中の本だけを表示します。表示できる本が無いグループはカードに出ません。
- 詳細モーダル (`static/js/detail.js`) は `getAllItems()` 経由でシリーズ・大シリーズの項目を解決するため、どちらのページでも表示中の分類が一致します。

静的アセットを追加した場合は `build.rs` の `html_files` / `js_files` にも登録してください。登録漏れがあるとキャッシュバスタ用ハッシュが更新されません。Worker は `build_assets.mjs` が `static/` をそのまま複製するため、追加作業はありません。

### 読書リーダー (Bibi)

EPUB リーダーには [Bibi](https://github.com/Rumia-Channel/bibi) **2.1.0** を使用します。[`bibi.lock.json`](bibi.lock.json) に SHA・lock/license の digest・Node/Bun を固定し、`scripts/import_bibi.mjs` で検証済み成果物を `static/vendor/bibi/` に取り込みます。CI は Bibi を一度ビルドし、同じ artifact を Native/Worker と各デプロイに渡します。

- 一覧・本一覧・シリーズ内の本をクリックすると、同じタブの `/reader/?book=<id>` で書籍情報を確認し、`/vendor/bibi/` の **Bibi 専用ページ**を開きます。Jo / iframe にリーダーを埋め込まず、一覧ナビや外側のツールバーも表示しません。カードの「詳細」は従来の詳細画面を開き、詳細・編集画面の「読む」も同じ起動経路を使用します。CD・オーディオブックのクリック動作は変更していません。
- 起動ページを `location.replace` で置き換えるため、一覧から開いた後はブラウザの「戻る」で直接元の一覧に戻れます。目次・ページ送り・表示設定・スライダーは Bibi 自身の UI を使用します。API ログインは共通の `static/js/auth.js` が担当し、起動時の書籍 API と同一 origin の EPUB 配信に既存の認証を引き継ぎます。
- EPUB 未登録の場合は登録案内を表示します。PDF は Bibi で開かず、ダウンロードを案内します。
- 再開位置は Bibi がブラウザ内に保存します。書籍 ID・ファイル版で分離しますが、サーバーへの位置保存、端末間同期、読了状態の自動更新は未接続です。

取り込みコマンド、API 契約、確認済み範囲と残作業は [docs/BIBI_INTEGRATION.md](docs/BIBI_INTEGRATION.md) を参照してください。
