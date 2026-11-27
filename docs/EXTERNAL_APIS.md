# 外部 API・メタデータ連携仕様

Dantalian が書籍・CD・オーディオブックのメタデータ取得および Web 小説表示に利用する外部サービスとの連携仕様です。

---

## 1. 国立国会図書館サーチ SRU（書籍メタデータ）

ISBN 登録時に書誌情報を取得する第一候補です。

- **エンドポイント**: `https://ndlsearch.ndl.go.jp/api/sru`
- **パラメータ**: `operation=searchRetrieve&version=1.2&recordSchema=dcndl_v3&onlyBib=true&maximumRecords=10&startRecord=1&recordPacking=xml&query=isbn%3D%22{isbn}%22`
- **仕様**: SRU 1.2 / DC-NDL（RDF）ver.3.0（Native / Worker 共通）
- **参考資料**:
  - [国立国会図書館サーチ API 利用案内](https://ndlsearch.ndl.go.jp/help/api)
  - [外部提供インタフェース仕様書 第1.5版](https://ndlsearch.ndl.go.jp/file/help/api/specifications/ndlsearch_api_20260925.pdf)
  - [DC-NDL ver.3.0](https://ndlsearch.ndl.go.jp/renkei/dcndl/version3)

### 検索・候補選択の挙動

1. **ISBN-10 / ISBN-13 の直列検索**: 入力された形式を先に検索し、書誌が見つかれば即座に採用します。未検出・通信失敗・パース失敗の場合のみ相互変換した別形式を検索します（ISBN の OR クエリは実サービスで `illegal query syntax` になるため使用しません）。
2. **候補の選択**: 1 回の検索で最大 10 件を取得し、著者典拠 ID の有無、NDL 自身の書誌、メタデータの充足度を基に最良候補を選びます。
3. **取得フィールド**: タイトル・読み、著者・読み・NDL 典拠 ID、出版社、出版日、解説、シリーズ名・読み、別タイトル・読み、巻次・読み、価格、ページ数、全国書誌番号（JPNO）、NDL URL。
4. **パース**: `#material` の書誌情報のみを対象とし、管理情報・個体情報の値を混入させません。SRU 診断エラーや途中で切れたレコードは「検索結果なし」と区別します。

---

## 2. Web NDL Authorities SPARQL 1.1（著者典拠）

書誌から NDL 典拠 ID を得られなかった著者の典拠 ID・読みを補完します。

- **エンドポイント**: `https://id.ndl.go.jp/auth/ndla/sparql`（[公式案内](https://id.ndl.go.jp/information/sparql-11/)）
- **方式**: URL エンコードした `query` と `format=json` を POST 送信（旧 `output=json` は不使用）
- **一括検索**: 典拠 ID が不足する著者だけを最大 50 名ずつ `VALUES` 句にまとめて 1 クエリで照会します。
- **対応付けと失敗時**: 結果に位置情報を含めて応答順に依存せず対応付け、既存の典拠 ID は上書きしません。同名候補が一意に特定できない場合や他機関 URI は採用しません。典拠補完が失敗しても、取得済みの書籍・著者情報は保持します。

---

## 3. Open Library Search API（NDL 未検出時のフォールバック）

- **エンドポイント**: `https://openlibrary.org/search.json`（[Search API](https://openlibrary.org/dev/docs/api/search) / [利用ガイド](https://openlibrary.org/developers/api) / [ライセンス](https://openlibrary.org/developers/licensing)）
- **切り替え条件**: NDL で ISBN-10 / ISBN-13 の**両方**が未検出（0 件または SRU 診断 `info:srw/diagnostic/1/1`「Record does not exist」）の場合のみ使用します。通信失敗・タイムアウト等を「未登録」と誤認してフォールバックしません。
- **検索と版の照合**: `isbn:(...)` で最大 5 作品を取得し、親作品全体ではなく入力／変換済み ISBN に一致する `editions.docs` のタイトル・出版社・刊行日を採用します（著者名は作品の `author_name` を使用）。Open Library の Covers API は呼び出しません。
- **登録ソースと日付**: 新規登録時の `source` は `"open_library"`。JPNO・NDL URL・NDL 典拠 ID は未設定とし、説明・表紙の Amazon 補完は維持します。日付は `YYYY-MM-DD` / `YYYY-MM-NN` / `YYYY-NN-NN` 形式へ正規化します。
- **識別・利用条件**:
  - `User-Agent` に Dantalian のバージョンとプロジェクト URL を付与します。環境変数 `OPEN_LIBRARY_CONTACT` を設定すると同ヘッダーに公開連絡先を追加します。
  - 利用者の ISBN 登録操作時のみ呼び出し、登録済み ISBN は DB キャッシュを再利用します。HTTP 429 / 5xx や不正 JSON はエラーとして返します（自動リトライや IPv4 切替は行いません）。

---

## 4. NDL / Open Library 共通のアクセス制御

外部 API への負荷を避けるため、NDL 書誌検索・NDL 典拠検索・Open Library 検索は共通の直列制御を通ります。

| 項目 | 仕様 |
| --- | --- |
| 送信間隔 | 各リクエスト送信前に最低 **2 秒**待機（初回・失敗・キャンセル後の次回も含む）。前の応答本文の取得完了まで次を送らない |
| Native の直列化 | プロセス内非同期ロック + OS 一時ディレクトリ内の `dantalian-ndl-request.lock` によるファイルロック。ロック取得失敗時は迂回せずエラー |
| Worker の直列化 | `NDL_COORDINATOR` にバインドした `NdlCoordinator` Durable Object（固定名 `ndl`、マイグレーション `ndl-coordinator-v1`）へ集約。バインド欠如時はエラー |
| タイムアウト | 応答本文を含め Native は **30 秒**、Worker は **10 秒**（タイムアウト・キャンセル時は外部通信も中止） |
| 制御の範囲 | 別マシン・別 OS ユーザー・異なる Worker デプロイ・Native/Worker 間ではロックを共有しないため、併用時は運用側で多重アクセスを避ける |

### 契約回帰テスト

```sh
# Worker ビルド後に実行（外部 API へは接続せずローカルフィクスチャで検証）
node --test worker/tests/ndl_api_contract.mjs
```

---

## 5. Amazon.co.jp（表紙画像・タイトル補完）

NDL / Open Library には表紙画像が含まれないため、商品ページから紙版表紙を取得します（Native では `scraper` + `tokio::task::spawn_blocking` を使用）。

1. **検索ページ**: `https://www.amazon.co.jp/s?k={isbn}` から `[cel_widget_id^="MAIN-SEARCH_RESULTS-"]` 内の候補を収集し、Kindle / Audible / 電子書籍を除外して紙版候補を優先します。詳細ページ上の `tmm-grid-swatch-*` で紙版が選択されていれば紙版候補として扱います。
2. **年齢確認ページ対応**: HTML に `black-curtain-verification` が含まれる場合、`/black-curtain/save-eligibility/black-curtain?returnUrl={/dp/パス}` を経由してセッションクッキーを更新したうえで再取得します（`cookie_store(true)`）。
3. **画像 URL 抽出優先順**:
   1. `img#landingImage` の `data-old-hires`
   2. `img#landingImage` の `data-a-dynamic-image`（JSON キーのうち文字列長が最長のもの）
   3. `img#imgBlkFront` の `src`
4. **保存**: SHA3-256（URL-safe Base64）+ 拡張子（`Content-Type` から判定）で保存し、既存ファイルがあれば再ダウンロードをスキップします。

---

## 6. DLsite（書籍・CD・オーディオブック / ASMR）

`POST /api/dlsite` により DLsite 作品 ID（英字 2 文字 + 数字 6〜8 桁、または作品 URL）から書誌・トラック情報を登録します。

- **種別の明示**: `media_type`（`book` / `cd` / `audiobook`）の指定が必須です（ID 接頭辞からの自動推測は行いません）。CD とオーディオブックの登録済み種別が競合する場合は `409` を返します。
- **取得対象**:
  - 作品情報 JSON（`/home/product/info/ajax?product_id=...`）および作品ページ HTML（`/home/work/=/product_id/...`）からタイトル、サークル（出版社欄へ保存）、販売日、説明、クレジット、表紙を取得します。
  - 作者・著者・原作・イラスト・シナリオ・声優・音楽等の有効なクレジットがない場合のみ、サークル名を著者にも補完します。声優は音声作品のアーティスト欄にも保存します。
  - 音声作品のトラックリストを掲載順に取り込みます（時間未記載は未設定、トラックリスト未掲載の作品は 0 件のまま保存し、ダミートラックは生成しません）。
- **音声の扱い**: 無料公開・試聴・サンプル音声および購入済み音声ファイル自体は一切取得・保存しません。実際の音声ファイルは利用者が後からアップロードします。

---

## 7. Web 小説（narou.rs）ブラウザ直接連携

管理画面の **設定 → Web 小説（narou.rs）** から接続先と対象タグを設定することで、通常の本一覧（`/`・`/books/`）に Web 小説を統合表示し、Bibi で閲覧できます。

### 設計方針

- Dantalian のサーバー（DB・ローカルディスク・D1・S3）には narou.rs の書誌情報・EPUB・長期トークンを一切保存しません。
- 書誌情報・お気に入りタグの取得および EPUB 本体の取得は、**利用者のブラウザが narou.rs へ直接アクセス**して行います（既存の narou.rs ログイン Cookie を利用）。

### 設定項目

| 設定キー | 説明 | 既定値 |
| --- | --- | --- |
| `narou.url` | ブラウザから到達できる narou.rs の HTTPS URL（Native ループバックのみ HTTP 可、パス接頭辞可） | 空（無効） |
| `narou.use_favorites` | narou.rs のお気に入りタグを対象に含めるか | `true` |
| `narou.tags` | 対象タグの条件の JSON 配列（UI では 1 行に 1 条件）。行どうしは OR、行の中の空白区切りは AND、`|` は OR、先頭の `-` は除外。空白を含むタグは `"..."` で囲む | `[]` |

### 動作・制約

- **CORS 設定**: narou.rs 側の `external-api.allowed-origins` に Dantalian の Origin（スキーム・ホスト・ポート、パスなし）を登録しておく必要があります（`/api/read/v1` が credentialed CORS を返します）。
- **同一サイト Cookie**: ローカル Native 同士を連携する場合は、両方 `127.0.0.1` または両方 `localhost` で開いてください。
- **対象作品の絞り込み**: タグ条件は narou.rs 側のクエリ（`/api/read/v1/novels` の `tag` / `not_tag` パラメータ）で絞り込みます。条件行ごとに 1 クエリを発行し、作品 ID の和集合を表示します（全件取得は行いません）。`narou.use_favorites` のお気に入りタグは 1 タグ 1 条件の OR として追加されます。タグの照合は完全一致です。この絞り込みには繰り返し `tag` / `not_tag` を解釈できる narou.rs（0.6.14 以降相当）が必要で、古い narou.rs では `|` を含む候補や除外が効かず結果が空になります。表紙は空欄とし、著者名で通常書籍と同じグループに表示します。
- **エンドポイント**:
  - `GET /api/narou/connection`: 設定済みの接続先 URL と対象タグのみ返します。
  - `GET|HEAD /api/narou/novels/{id}/epub`: 設定済み narou.rs の `/api/read/v1/novels/{id}/epub` へ `302` リダイレクトするのみで、本文を中継・保存しません。
- **Worker デプロイの補足**: 同一 Cloudflare zone の Route 上にある narou.rs へ公開側 Worker / Access 経由で接続できるよう、`wrangler.toml` では `global_fetch_strictly_public` 互換フラグを有効にしています。
