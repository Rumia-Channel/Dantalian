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

## v0.2.4

- API の15秒キャッシュと更新後30秒だけ primary を優先する cookie を廃止しました。Worker の読み取りは `first-primary` で開始し、Native / Worker の API 応答とブラウザの API 取得は `no-store` にします。バージョン付き静的アセットのキャッシュは維持します。
- 登録・編集後の別タブと、ブラウザの「戻る」で復元された一覧を更新します。非表示タブの再取得は表示時まで遅延し、復元時の `storage` / `visibilitychange` / `pageshow` はまとめて処理します。ポーリングや D1 の更新番号テーブルは追加しません。
- 書籍・CD の編集画面は、対象1件と必要な選択肢だけを取得します。Native / Worker に `GET /api/cds/:id` を追加し、全作品一覧と同じトラックの重複取得をやめました。未保存の編集フォームを別タブの変更で自動再読み込みすることはありません。
- Worker のトラックメタデータ取得を1 SQLに統合し、親の照合・CDからの継承・技術タグ・カバー BLOB を保持します。著者の正規化照合は登録1回につき最大1スキャンにまとめ、書籍一覧の代表著者は取得済みの著者配列から求めます。
- 大シリーズ・プレイリスト一覧は、読み取り行数が増える相関クエリ化を避け、2 SQLを1回の D1 バッチにまとめます。プレイリストの並び替えは位置が変わる行だけを更新し、重複追加・対象のない削除では書き込みません。
- DB スキーマ・インデックス・依存ライブラリの追加変更はありません。既存データの移行は不要です。

### D1 の検証範囲

100 / 1,000 / 10,000件の書籍・CD・著者の固定モデルで、変更前後の実際の Worker Wasm を起動して比較しました。
各作品に著者1名、各書籍に1コピーと返却済み貸出1件、各CDに1トラック、大シリーズに10書籍、プレイリストに10トラックを登録しています。
下表は10,000件時の、キャッシュを使わない Miniflare D1 の `meta.rows_read` / `meta.rows_written` と実行回数です。本番アカウントの課金実績・料金保証ではありません。

| 処理 | 指標 | v0.2.3 | v0.2.4 |
| --- | --- | ---: | ---: |
| CD編集で対象を選択 | 読取行 | 139,996 | 16 |
| 書籍一覧100件の1ページ | 読取行 | 1,009 | 707 |
| 既存著者5名の一括照合 | 読取行 | 50,008 | 10,008 |
| 10曲のプレイリストから末尾を削除 | 書込行 | 20 | 2 |
| 残り8曲で対象のないトラック削除 | 書込行 | 17 | 0 |
| 登録済みトラックの重複追加 | 書込行 | 1 | 0 |
| CDトラックのメタデータ取得 | SQL文 | 5 | 1 |
| 大シリーズ・プレイリスト一覧（各API） | D1往復 | 2 | 1 |

バッチは往復を減らしますが、SQL文数や課金対象の読取行を自動的に減らすものではありません。一覧の相関クエリ化は読取行が増えたため採用せず、大シリーズ42,000行・プレイリスト32,000行を維持しています。書籍の1ページは総件数1,000 / 10,000件とも707行で、全表読み取りにはなっていません。

全件検索・グループ表示の既存仕様により、一覧画面は書籍の全ページとCD・プレイリストの全項目を取得します。編集画面ではその全件取得を行いません。著者のUnicode正規化照合も、登録ごとの1スキャンは残ります。追加インデックスによる恒常的な書込行増加はありません。

## v0.2.3

- Bibi を 2.3.2 へ更新しました。PDF・画像ZIPの読み方向を本・ユーザー・ファイル版ごとに保存し、開き直した際に復元します。
- 画像ZIPは見開きの両ページをまとめて先読みし、読み込み順序の変更時も描画済みページを再読み込み・消去しません。
- Worker の音声アップロードに Native 用の既定100 MB上限が適用され、S3直接送信前に拒否される問題を修正しました。Native の設定上限は維持し、Worker の設定画面には直接送信方式を表示します。

## v0.2.2

- トラック編集は「ディスク番号 → トラック番号 → トラックタイトル → 時間」の順に表示し、ディスク番号とトラック番号を直接変更できます。
- 番号を片方だけ変更しても、もう片方の番号・タイトル・時間を保持します。新しいトラックは、そのディスク内の最大トラック番号の次へ追加します。
- 音声アップロード時、時間が空欄または空白だけなら取得した長さを確認なしで保存します。既に時間が入力されている場合は上書き確認を維持します。
- Bibi を 2.3.1 へ更新しました。画像だけの ZIP を書籍として開けるようになり、自然順にページを並べます。EPUB・PDF の位置・巻間ナビゲーション API は変更ありません。
- トラック編集は既存の DB 列だけを使い、追加マイグレーションは不要です。

## v0.2.1

- Worker の書籍ファイル（EPUB・ZIP・PDF）は、サイズによらずブラウザから Wasabi へ直接アップロードします。従来は95MiB以下の書籍ファイルを Worker が受け取っていました。
- 直送経路のエラー時に Worker 本文送信へ切り替える処理を廃止しました。実行環境の確認に失敗した場合も送信を中断し、次の手動操作で再確認します。
- v0.2.0 からの追加DBマイグレーションはありません。音声の直送条件（8MiB以上）とカバー画像の送信経路は従来どおりです。

## v0.2.0

- Bibi を 2.3.0 へ更新し、書籍に複数の EPUB・ZIP・PDF を追加・個別削除できるようにしました。書籍カードの ID 表示・コピーにも対応しています。
- DLsite ID と登録種別（書籍・CD・オーディオブック／ASMR）を指定し、本家から作品情報・表紙・声優・曲目情報を取得できます。無料公開・試聴・サンプル音声は取得・登録しません。
- トラックリスト未掲載の作品はトラック0件、時間未記載の曲目は時間未設定で保存します。ファイル追加・削除時も未保存の編集内容を保持します。
- Native はスキーマ17へ自動移行します。Worker は D1 マイグレーション `0019`～`0021` を順に適用し、既存の単一添付を `book_files` へ引き継ぎます。本番CIは適用前の D1 Time Travel 復元ポイントを記録します。


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
- DLsite ID による書籍・CD・オーディオブック／ASMR の登録（本家メタデータ・表紙・作品のトラックリスト）
- ユーザー定義シリーズ管理（作成・変更・削除・書籍への割り当て）
- 本だけを扱う一覧（`/books/`）。著者別／一覧の切り替え、検索、並び順、シリーズ・大シリーズの絞り込みに対応
- 書籍カードの書籍 ID 表示・コピー（CD・音声作品の親書籍 ID の指定に利用）
- 書籍ごとの複数 EPUB・ZIP・PDF の追加、個別削除、ダウンロード、Bibi での閲覧
- レスポンシブ対応ダークテーマ UI
- コンテンツ幅設定（LocalStorage に保存）

### 書籍ファイル

編集画面の「ファイルを追加」では複数ファイルを選択できる。追加時に既存ファイルを置き換えず、詳細・編集画面に各ファイルの「読む」「ダウンロード」と、編集画面に個別の削除操作を表示する。本一覧のカードからは最初に登録したファイルを開く。ファイルの追加・削除ではファイル欄だけを更新し、保存前の書籍情報の入力を保持する。

Native / Worker 共通で `POST /api/books/{id}/files` にファイルを追加し、`DELETE /api/books/{id}/files/{file_id}` で1件だけ削除する。書籍詳細 API は `files` 配列（`id`, `book_id`, `file_hash`, `file_name`, `created_at`）を返す。従来の単一ファイルは `book_files` へ移行し、他の書籍が同じファイルを参照している場合は実ファイルを削除しない。Worker のブラウザ UI は全書籍ファイルを presigned multipart で直送し、`POST /api/books/{id}/files/finalize` でアップロード済みオブジェクトを追加する。`object_kind: "epub"` と `epubs/` は EPUB・ZIP・PDF 共通の内部区分・保存先であり、EPUB 専用ではない。

ZIP は Bibi 2.3.2 で EPUB 構成の ZIP に加えて、画像だけを含む ZIP も書籍として開けます。画像は PNG・JPEG・GIF・WebP・AVIF・BMP に対応し、自然順（`1`,`2`,`10`）とサブフォルダの順でページになります。EPUB 構成・BibiZine・画像ブックの順に判定し、対応しない ZIP は開けません。PDF・画像ZIPで選んだ左右の読み方向は書籍ごとに保存・復元します。

### DLsite 登録

登録画面のタブは ISBN → ISDN → CD → オーディオブック → DLsite → 手動。DLsite では種別（書籍・CD・オーディオブック／ASMR）と作品 ID の両方を必須とする。作品 ID の接頭辞から種別を推測しない。

- Native / Worker 共通の `POST /api/dlsite` に `{"media_type":"audiobook","dlsite_id":"RJ01722318"}` を送る。種別は `book` / `cd` / `audiobook`。書籍は `book`、CD・音声作品は `cd` キーで保存結果を返し、`source` は新規取得なら `dlsite`、登録済みなら `cache`。
- ID は英字2文字＋数字6～8桁を受け付け、大文字へ正規化する。本家作品 URL も使用できる。この形式の検証は、存在する ID や接頭辞の網羅性を保証しない。
- ISBN・ISDN がない書籍も DLsite ID で保存・編集・検索できる。CD・音声作品にも独立した `dlsite_id` を保存し、一覧の検索・詳細の本家リンクで利用する。CD とオーディオブックの登録済み種別が異なる場合は `409` を返す。
- 本家の [作品情報 JSON](https://www.dlsite.com/home/product/info/ajax?product_id=RJ01722318) と [作品ページ](https://www.dlsite.com/home/work/=/product_id/RJ01722318.html) からタイトル、サークル、販売日、説明、クレジット、表紙を取得する。声優は音声作品のアーティスト欄にも保存する。書籍ではシリーズ・価格・ページ数も取得可能な範囲で保存する。
- 音声作品の `.work_parts.type_tracklist .work_tracklist_item` を掲載順に取り込み、`.title` と `.time` を既存トラックへ保存する。時間未記載は未設定、トラックリスト未掲載（例: `RJ01761229`）は **0件のまま** とし、作品名からダミートラックを生成しない。`MM:SS` / `HH:MM:SS` の表記を保持する。
- 音声データや購入コンテンツは取得しない。無料公開・試聴・サンプル音声を取得・登録せず、音声 URL も保存しない。作品のトラックリストから曲目情報だけを保存し、実際の音声ファイルは利用者が後からアップロードする。
- Native のスキーマは17、Worker は `0020_book_dlsite_id.sql` と `0021_cd_dlsite_id.sql` を含む D1 マイグレーションを適用する。公開前にマイグレーションし、CSS / JS のキャッシュバスタ更新のため Native と Worker のアセットをリビルドする。


## 環境変数

| 変数 | 説明 | デフォルト |
|------|------|------------|
| `DATA_DIR` | データ保存ディレクトリ | `~/Documents` (Windows: `C:\Users\{user}\Documents`) |
| `BIND_ADDR` | 待受アドレス。ループバック以外を指定する場合は `DANTALIAN_API_TOKEN` が必須 | `127.0.0.1` |
| `PORT` | サーバー待受ポート | `3000` |
| `DANTALIAN_API_TOKEN` | Native API の認証トークン。設定すると `/api`（`/api/health` を除く）・`/audio`・`/images`・`/epubs` に Bearer または `dantalian_api_token` Cookie を要求する | 未設定（ループバック利用時のみ認証なし） |
| `RUST_LOG` | ログレベル | `dantalian=debug` でデバッグ出力 |
| `OPEN_LIBRARY_CONTACT` | Open Library に送信する公開連絡先（メール等）。Worker では Vars に設定。変更後は再起動／再デプロイする | 未設定（プロジェクト URL のみ） |

> **ネットワーク公開について**: Native サーバーは既定で `127.0.0.1` にのみ待ち受けます。
> LAN などから利用する場合は `DANTALIAN_API_TOKEN` を設定したうえで `BIND_ADDR=0.0.0.0`
> などを明示してください。トークン未設定で非ループバックを指定した場合、起動を拒否します。
> ブラウザの API ログイン欄（`static/js/auth.js`）に同じトークンを入力すると、API・音声・
> 画像・EPUB の取得に認証が付与されます。

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

Native ランタイムでは DB の `settings` テーブル (管理画面「設定」→「アップロード」からも変更可能) で指定します。単位は MB。

| キー | デフォルト | 説明 |
|------|-----------|------|
| `upload.cover_max_mb` | `10` | カバー画像の上限 |
| `upload.audio_max_mb` | `100` | 音声ファイルの上限 |
| `upload.file_max_mb` | `500` | 書籍ファイル (epub/pdf/zip) の上限 |

上限は 4096MB (4GB) まで。これは**アプリ側**の上限です。前面のリバースプロキシ (nginx: `client_max_body_size` 等) や Cloudflare の上限も、これ以上に引き上げる必要があります (実効上限は両者の小さい方)。

Worker ランタイムでは、すべての書籍ファイル（EPUB・ZIP・PDF）と8MiB以上の音声を presigned multipart でブラウザから Wasabi へ直接アップロードするため、ファイル本文は Worker を通らず、Cloudflare のリクエストボディ上限（Free/Pro 100MB）を受けません。8MiB未満の音声とカバー画像は Worker を経由します。直送経路が失敗しても Worker 経由には切り替えません。実行環境を確認できない場合も Native と推測せず送信を中断します。

Worker の画面側でも、Native 専用の `upload.audio_max_mb`（既定100 MB）・`upload.file_max_mb`（既定500 MB）は適用しません。音声のメタデータはローカル WASM で解析し、大容量のファイル本文は Wasabi へ直接送信します。Worker の設定画面には Native 用の上限入力欄ではなく、この送信方式を表示します。

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

Native の現行スキーマは `18` です。対応版より新しい DB を開いた場合、テーブルを削除せず起動を拒否します。新しいバージョンで更新した DB を古い実行ファイルで開かないでください。

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

### narou.rs の Web 小説を同期表示

管理 → **Web 小説**で narou.rs の接続先と対象タグを保存します。手動の一度きりの取り込みではなく、narou.rs を正として **`/books/` を開く時・表示中の5分ごと・タブに戻った時**に API と同期します。トップの「一覧」には、シリーズ・大シリーズ内も含めて表示しません。

| 設定キー | 内容 |
|---|---|
| `narou.url` | narou.rs の HTTP(S) ベース URL。パス接頭辞も利用可能。Dantalian サーバーから到達できる URL を指定 |
| `narou.token` | Worker 版の Bearer token、または Native 版の `.narou/ui_token` |
| `narou.cf_access_client_id` | Cloudflare Access Service Token の Client ID |
| `narou.cf_access_client_secret` | Cloudflare Access Service Token の Client Secret |
| `narou.use_favorites` | narou.rs のお気に入りタグを対象にするか（既定 `true`） |
| `narou.tags` | Dantalian 側で追加指定するタグ名の JSON 配列。画面では1行1タグで指定 |

- 対象は **お気に入りタグと指定タグの和集合に完全一致**する小説です。タグが空なら全件取得せず、Web 小説を非表示にします。
- `GET /api/favorite_tags`・`GET /api/list?all=true` で最新状態を確認し、更新日時・書誌情報が変わった作品だけ `GET /novels/{id}/download` から EPUB を取得します。元の小説サイトを Dantalian がスクレイピングすることはありません。
- EPUB は既存のファイル保存先（Native: `epubs/`、Worker: Wasabi）へキャッシュし、本棚の本から既存の Bibi で読みます。narou.rs 側で EPUB の生成が可能な設定が必要です。Worker が中継する EPUB の上限は95 MiBです。
- 同じ作品の更新では書籍 ID・読書状態・手動追加ファイルを維持し、同期した EPUB だけを差し替えます。対象タグから外れた作品、削除済みの作品、以前の接続先の作品は非表示にし、DB の読書状態は保持します。同期失敗時は前回の内容とエラーを表示します。
- Bearer のみ、[Cloudflare Access Service Token](https://developers.cloudflare.com/cloudflare-one/access-controls/service-credentials/service-tokens/) のみ、両方の併用に対応します。Access 側は **Service Auth** ポリシーを使用してください。Client ID / Secret は両方必要です。秘密値は設定取得 API に返さず、未入力なら維持、削除チェックで解除します。
- 認証情報の送信には HTTPS が必要です（ループバック接続のみ HTTP 可）。URL 内の認証情報・クエリ・フラグメントと、リダイレクトは拒否します。
- `POST /api/narou/sync` で即時同期も可能です。結果は `matched` / `imported` / `updated` / `unchanged` / `tags` / `book_ids` / `failed`。`GET /api/books?exclude_narou=true` は同期小説を除外し、通常のページングを維持します。

Worker の更新時は `worker/migrations/0022_narou_imports.sql` まで D1 に適用してください。Native は起動時に移行します。


### 読書リーダー (Bibi)

EPUB リーダーには [Bibi](https://github.com/Rumia-Channel/bibi) を使用します。[`bibi.lock.json`](bibi.lock.json) にバージョン・SHA・lock/license の digest・Node/Bun を固定し、`scripts/import_bibi.mjs` で検証済み成果物を `static/vendor/bibi/` に取り込みます。CI は Bibi を一度ビルドし、同じ artifact を Native/Worker と各デプロイに渡します。

- トップページ (`/`) の本カードとシリーズ・大シリーズ内の本は、クリックしても読書ページへ遷移せず、従来の詳細画面を開きます。読書は詳細・編集画面の「読む」から開始します。CD・オーディオブックのクリック動作は変更していません。
- 本専用一覧 (`/books/`) の本カードとシリーズ・大シリーズ内の本は、同じタブの `/reader/?book=<id>` で書籍情報を確認し、document-pinned preset を持つ `/vendor/bibi/dantalian.html` の **Bibi 専用ページ**を開きます。Jo / iframe にリーダーを埋め込まず、一覧ナビや外側のツールバーも表示しません。カードの「詳細」は従来の詳細画面を開き、詳細・編集画面の「読む」も同じ起動経路を使用します。
- 起動ページを `location.replace` で置き換えるため、一覧から開いた後はブラウザの「戻る」で直接元の一覧に戻れます。目次・ページ送り・表示設定・スライダーは Bibi 自身の UI を使用します。API ログインは共通の `static/js/auth.js` が担当し、起動時の書籍 API と同一 origin の EPUB 配信に既存の認証を引き継ぎます。
- EPUB 未登録の場合は登録案内を表示します。PDF は Bibi で開かず、ダウンロードを案内します。
- 再開位置は Bibi がブラウザ内に保存します。書籍 ID・ファイル版で分離しますが、サーバーへの位置保存、端末間同期、読了状態の自動更新は未接続です。
- シリーズの本は、本文末尾からさらに進むと「次の巻へ」、表紙からさらに戻ると「前の巻へ」の案内を表示します。別の追加操作で隣接巻へ移動し、本文方向の操作で現在の本へ戻ります。隣接巻がない場合は元の一覧・編集ページへ戻ります。未登録・未対応の隣接巻を飛ばすことはありません。
- 目次の隣の「戻る」は、最初の起動元の query/fragment を保持して戻ります。巻間移動と戻るは Dantalian の明示設定だけで有効化し、URL の preset 指定だけでは許可しません。`GET /api/reader-navigation` は Native/Worker で同じ書籍・ファイル版・巻順を検証し、読書状態を書き換えません。

取り込みコマンド、API 契約、確認済み範囲と残作業は [docs/BIBI_INTEGRATION.md](docs/BIBI_INTEGRATION.md) を参照してください。
