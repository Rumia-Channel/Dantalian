# Tsukuyomi

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
| `RUST_LOG` | ログレベル | `tsukuyomi=debug` でデバッグ出力 |

## データ構成

```
{DATA_DIR}/
  Tsukuyomi/
    db/
      tsukuyomi.db    # SQLite データベース
    images/       # 表紙画像ファイル
```
