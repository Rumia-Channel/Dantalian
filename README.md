# Dantalian

ISBN から書籍メタデータを自動取得し、ローカルで管理する Web アプリケーション。

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
- **処理**: ウィジェット内の `<a href>` を全て取得し、以下の優先順位で商品ページ URL を決定
  1. `/dp/` を含み `/ebook/dp/` を含まないリンク（紙版優先）
  2. `/dp/` を含むリンク（Kindle 含む）
  3. 最初に見つかったリンク

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
