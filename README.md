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

### 国立国会図書館サーチ SRU（書籍メタデータ取得）

- **エンドポイント**: `https://ndlsearch.ndl.go.jp/api/sru?operation=searchRetrieve&version=1.2&recordSchema=dcndl_v3&onlyBib=true&maximumRecords=10&startRecord=1&recordPacking=xml&query=isbn%3D%22{isbn}%22`
- **仕様**: SRU 1.2 / DC-NDL（RDF）ver.3.0。Native / Worker で同じ形式を使用する。
- **ISBN10 / ISBN13**: 入力された形式を先に検索し、書誌が見つかれば即座に採用して終了する。未検出・通信失敗・パース失敗の場合だけ、相互変換したもう一方の形式を検索する。先の検索が失敗しても、もう一方で書誌を取得できれば成功とする。両形式とも未検出の場合だけ Open Library へ進み、書誌が得られず通信等の失敗が残る場合はエラーを返す。HTTP リクエストは必ず直列に送る。ISBN の OR 条件をまとめたクエリは実サービスで `illegal query syntax` となったため採用しない。
- **候補の選択**: 一つの検索で最大 10 件を取得し、著者典拠 ID、NDL 自身の書誌、メタデータの充足度を基に候補を選択する。書誌取得に成功した後、別形式との比較のためだけに追加検索することはない。
- **取得フィールド**: タイトル・読み、著者・読み・NDL 典拠 ID、出版社、出版日、解説、シリーズ名・読み、別タイトル・読み、巻次・読み、価格、ページ数、全国書誌番号（JPNO）、NDL URL。JPNO は識別子の型を確認し、ISBN や他機関の識別子と混同しない。
- **パース**: `quick-xml` で `#material` の書誌情報を読み、管理情報・個体情報の値を混入させない。SRU の診断エラーや途中で切れたレコードは「検索結果なし」と区別する。
- **公式資料**: [API 利用案内](https://ndlsearch.ndl.go.jp/help/api)、[外部提供インタフェース仕様書 第1.5版](https://ndlsearch.ndl.go.jp/file/help/api/specifications/ndlsearch_api_20260925.pdf)、[DC-NDL ver.3.0](https://ndlsearch.ndl.go.jp/renkei/dcndl/version3)。

### Web NDL Authorities SPARQL 1.1（著者典拠取得）

- **エンドポイント**: `https://id.ndl.go.jp/auth/ndla/sparql`。URL エンコードした `query` と `format=json` を POST する。旧エンドポイントの `output=json` は使用しない。
- **一括取得**: Native / Worker とも、NDL 典拠 ID が不足する著者だけを最大 50 名ずつ `VALUES` でまとめて検索する。HTTP の同時送信ではなく、一つのクエリで複数名を検索する方式。
- **対応付け**: 著者ごとの位置を結果に含め、応答順に依存せず元の著者に対応付ける。既知の ID は上書きしない。同名候補を一意に特定できない場合や他機関の URI は、NDL 典拠 ID として採用しない。
- **失敗時**: 典拠の補完に失敗しても取得済みの書籍情報・著者情報は保持する。NDL のデータ更新等で利用できない時間帯は、[公式の SPARQL 1.1 案内](https://id.ndl.go.jp/information/sparql-11/)を参照する。

### Open Library Search API（NDL 未検出時の書誌フォールバック）

- **エンドポイント**: `https://openlibrary.org/search.json`。[公式の Search API](https://openlibrary.org/dev/docs/api/search)を使用し、Legacy とされる `/api/books` や HTML スクレイピングは使用しない。
- **切り替え条件**: Native / Worker とも、NDL の ISBN10 / ISBN13 が両方とも未検出の場合だけ利用する。0 件の応答に加え、NDL 仕様書の表 3-5 に定義された `Record does not exist`（URI `info:srw/diagnostic/1/1`）も未検出として扱う。URI とメッセージの組み合わせで判別し、通信失敗・タイムアウト・その他の SRU 診断を「未登録」に置き換えることはない。
- **検索**: ISBN10 / ISBN13 を一つの `isbn:(...)` クエリにまとめ、必要な `fields` だけを指定して最大 5 作品を取得する。版情報の取得に必要な親作品の `key` も指定する。
- **版の照合**: 作品全体のタイトル・出版社・初版年ではなく、ISBN が入力または相互変換した ISBN と一致する `editions.docs` のタイトル・出版社・刊行日を採用する。著者名は作品の `author_name` を使用する。別の版の書誌を流用しない。
- **日付**: 刊行日は ISBN の一致を確認できた Amazon の日付を優先し、取得できなかった場合は NDL / Open Library の日付を使用する。年月日を別ソースから継ぎ合わせない。書誌の日付は英語の月名を含む値も `YYYY-MM-DD` / `YYYY-MM-NN` / `YYYY-NN-NN` に正規化し、年・月まで判明している値の精度を保持する。
- **取得元**: 新規登録の応答は `source: "open_library"`。Open Library の著者を NDL の典拠 ID と混同せず、NDL URL・JPNO も未設定とする。説明と表紙の Amazon 補完は従来どおり維持する。Open Library の Covers API は呼び出さない。
- **未検出・障害**: 一致する版がなければ未検出として扱う。HTTP 429 / 5xx や不正 JSON は上流エラーとして返し、未検出に置き換えない。Open Library のエラーに対する自動再試行・IPv4 切り替えは行わない。
- **利用方式**: 利用者の ISBN 登録操作に応じた検索に限定し、大量収集や複数 IP へのアクセス分散は行わない。登録済み書誌は既存の DB キャッシュを再利用し、同じ保存済み ISBN を再取得しない。
- **識別・利用条件**: `User-Agent` に Dantalian のバージョンとプロジェクト URL を指定する。継続利用時の公開連絡先は `OPEN_LIBRARY_CONTACT` に設定すると同ヘッダーに追加される。未設定時も未識別リクエストの上限（1 リクエスト/秒）を超えないよう、下記の共通制御で毎回 2 秒以上待機する。アプリ登録・連絡先の案内、キャッシュ・大量利用の制約は[公式の利用ガイド](https://openlibrary.org/developers/api)に従う。連絡・登録を自動送信することはない。
- **ライセンス**: [Open Library のライセンス案内](https://openlibrary.org/developers/licensing)を参照。メタデータの利用と書籍本文・画像の権利を同一視しない。

### NDL / Open Library 共通のアクセス制御・利用条件

- **間隔**: NDL 書誌検索・典拠検索・Open Library 検索を同じ制御で直列化し、毎回送信前に最低 2 秒待機する（初回も対象）。前の応答本文の取得が終わるまで次を送らず、失敗・キャンセル後にも次の送信前の待機を適用する。2 秒は本アプリの保守的な設定であり、NDL が公表した上限・推奨値ではない。
- **Native**: 非同期のプロセス内ロックと、OS の一時ディレクトリ内の `dantalian-ndl-request.lock` によるファイルロックを併用する。同じ OS ユーザー・一時ディレクトリを使う複数プロセス間でも多重送信を防ぐ。ロックファイルを取得中に削除しない。ロックを開けない場合は直接通信へ迂回せずエラーにする。
- **Worker**: `NDL_COORDINATOR` にバインドした `NdlCoordinator` Durable Object の固定名 `ndl` に NDL と Open Library の書誌通信を集約する。同じデプロイの複数 API isolate 間でも直列化する。ローカル・develop・staging・production の Wrangler 設定にバインドと `ndl-coordinator-v1` マイグレーションを含めてデプロイする。バインドがない古い設定では取得をエラーにし、isolate 内だけの制御や直接アクセスへフォールバックしない。
- **タイムアウト**: 応答本文を含む NDL / Open Library 通信に Native は 30 秒、Worker は 10 秒の上限を設ける。Worker はタイムアウト・キャンセル時に外部通信も中止する。Native の NDL エラーに対する既存の IPv4 再取得のみ維持し、Open Library エラーでは再取得しない。
- **制御の範囲**: 別マシン・別ユーザー・別一時ディレクトリの Native、異なる Worker デプロイ、Native と Worker の間ではロックを共有しない。それらを併用する場合も各 API への多重アクセスを生じさせない運用が必要。
- **クレジット**: アプリ共通フッターに NDL サーチ API、Web NDL Authorities API、Open Library API の利用を明記し、公式案内へリンクする。他のデータ提供機関の書誌には別の利用条件があるため、[API 提供対象データプロバイダ一覧](https://ndlsearch.ndl.go.jp/help/api/provider)も確認する。
- **継続利用**: NDL は継続的な利用について連絡先・利用内容の通知への協力を求めている。営利利用等で必要な申請・許諾と併せて、[公式の利用案内](https://ndlsearch.ndl.go.jp/help/api)に従う。アプリから連絡・申請を自動送信することはない。

ローカル回帰検証は Worker をビルドした後に `node --test worker/tests/ndl_api_contract.mjs` で実行する。外部 API にはアクセスせず、ISBN10 / ISBN13 片方だけの資料、書誌取得成功後の追加検索の停止、最初の検索の失敗から別形式での取得成功、一括典拠取得、異なる API isolate 間の直列化、遅い応答本文、取得失敗とタイムアウト後の継続、Open Library へのフォールバック・版の照合・キャッシュ・エラー区別を検証する。

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

- ISBN による書籍登録（NDL メタデータ、未検出時は Open Library、Amazon 表紙画像）
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
| `OPEN_LIBRARY_CONTACT` | Open Library に送信する公開連絡先（メール等）。Worker では Vars に設定。変更後は再起動／再デプロイする | 未設定（プロジェクト URL のみ） |

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

Worker ランタイムでは、音声 (8MiB 以上) と EPUB (95MiB = 99.6MB を超えるもの) は presigned multipart でブラウザから Wasabi へ直接アップロードするため、Cloudflare のリクエストボディ上限 (Free/Pro 100MB) を受けません。小さいファイルとカバー画像は Worker を経由します。

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

EPUB リーダーには [Bibi](https://github.com/Rumia-Channel/bibi) **2.2.2** を使用します。[`bibi.lock.json`](bibi.lock.json) に SHA・lock/license の digest・Node/Bun を固定し、`scripts/import_bibi.mjs` で検証済み成果物を `static/vendor/bibi/` に取り込みます。CI は Bibi を一度ビルドし、同じ artifact を Native/Worker と各デプロイに渡します。

- トップページ (`/`) の本カードとシリーズ・大シリーズ内の本は、クリックしても読書ページへ遷移せず、従来の詳細画面を開きます。読書は詳細・編集画面の「読む」から開始します。CD・オーディオブックのクリック動作は変更していません。
- 本専用一覧 (`/books/`) の本カードとシリーズ・大シリーズ内の本は、同じタブの `/reader/?book=<id>` で書籍情報を確認し、document-pinned preset を持つ `/vendor/bibi/dantalian.html` の **Bibi 専用ページ**を開きます。Jo / iframe にリーダーを埋め込まず、一覧ナビや外側のツールバーも表示しません。カードの「詳細」は従来の詳細画面を開き、詳細・編集画面の「読む」も同じ起動経路を使用します。
- 起動ページを `location.replace` で置き換えるため、一覧から開いた後はブラウザの「戻る」で直接元の一覧に戻れます。目次・ページ送り・表示設定・スライダーは Bibi 自身の UI を使用します。API ログインは共通の `static/js/auth.js` が担当し、起動時の書籍 API と同一 origin の EPUB 配信に既存の認証を引き継ぎます。
- EPUB 未登録の場合は登録案内を表示します。PDF は Bibi で開かず、ダウンロードを案内します。
- 再開位置は Bibi がブラウザ内に保存します。書籍 ID・ファイル版で分離しますが、サーバーへの位置保存、端末間同期、読了状態の自動更新は未接続です。
- シリーズの本は、本文末尾からさらに進むと「次の巻へ」、表紙からさらに戻ると「前の巻へ」の案内を表示します。別の追加操作で隣接巻へ移動し、本文方向の操作で現在の本へ戻ります。隣接巻がない場合は元の一覧・編集ページへ戻ります。未登録・未対応の隣接巻を飛ばすことはありません。
- 目次の隣の「戻る」は、最初の起動元の query/fragment を保持して戻ります。巻間移動と戻るは Dantalian の明示設定だけで有効化し、URL の preset 指定だけでは許可しません。`GET /api/reader-navigation` は Native/Worker で同じ書籍・ファイル版・巻順を検証し、読書状態を書き換えません。

取り込みコマンド、API 契約、確認済み範囲と残作業は [docs/BIBI_INTEGRATION.md](docs/BIBI_INTEGRATION.md) を参照してください。
