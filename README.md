# Dantalian

書籍・CD・オーディオブックの蔵書管理、電子書籍リーダー（EPUB / PDF / 画像 ZIP）、および音声プレイヤーを統合したセルフホスト向けメディアライブラリです。

ローカル環境で単体動作する **Native サーバー（Rust / Axum + SQLite）** と、**Cloudflare Workers + D1 + Wasabi** 構成のサーバーレスデプロイの両方に対応しています。

---

## 主な機能

### メディア登録・メタデータ自動取得
- **書籍（ISBN / ISDN）**: 国立国会図書館サーチ（NDL Search SRU）から書誌情報・Web NDL Authorities から著者典拠 ID を取得。未検出時は Open Library にフォールバックし、Amazon.co.jp から表紙画像を補完します。バーコードリーダー向けの一括登録にも対応。
- **CD / オーディオブック**: JAN / ISBN による MusicBrainz・Discogs・Amazon 照合と候補選択。トラック一覧・技術メタデータ・カバー画像を管理できます。
- **DLsite 連携**: 作品 ID または作品 URL と種別（書籍 / CD / オーディオブック）を指定して、作品情報・表紙・声優・トラックリストを取り込みます（音声・試聴データは取得しません）。
- **手動登録**: 識別子のない同人誌や私家版なども手動で登録・編集できます。

### 蔵書・コレクション管理
- **シリーズ / 大シリーズ**: 巻数付きのシリーズ管理と、複数シリーズ・書籍・CD を束ねる大シリーズ管理。
- **分類・保管**: 階層対応の保管場所、レーベル、アーティスト統合（名寄せ）、所蔵コピーと貸出・返却履歴の管理。
- **一覧・検索**: 全体一覧（`/`）、本専用一覧（`/books/`）、音楽・音声一覧（`/music/`）、アーティスト一覧（`/authors/`）。タイトル・著者・シリーズ・大シリーズによる絞り込み・並び替えに対応。

### 電子書籍リーダー（Bibi）
- **[Bibi 2.3.6](https://github.com/Rumia-Channel/bibi)** をスタンドアロン構成で統合し、EPUB に加えて **PDF** および **画像 ZIP**（自然順・サブフォルダ対応）を閲覧できます。
- 1 冊の書籍に対して複数の電子書籍ファイルを添付・個別ダウンロード・削除でき、シリーズ間の前後巻ナビゲーションにも対応しています。
- ブラウザ内 **ローカル EPUB キャッシュ（IndexedDB）** と、PDF / 画像 ZIP 用のデフォルト読み方向設定（右読み／左読み）を管理画面から設定できます。
- **Web 小説（narou.rs）連携**: サーバーに書誌・EPUB を保存せず、利用者のブラウザから narou.rs へ直接アクセスして通常一覧に統合表示・閲覧できます。

### 音楽・オーディオブック再生
- **プレイヤー & プレイリスト**: キュー再生、プレイリスト作成・並び替え、ブラウザ内オフライン音声キャッシュ、iOS / iPadOS を含む音量制御に対応。
- **省データ再生**: 対象拡張子の音声をバックグラウンドで Opus (Ogg) / AAC (ADTS) へ変換し、再生時の通信量を削減します（Native はアプリ内ワーカー、Worker は Cloudflare Queue + Container で非同期処理）。

### 運用・UI
- **テーマ & PWA**: OS 設定に追従するライト／ダーク／自動テーマ、レスポンシブ UI、PWA インストール対応。
- **認証**: `DANTALIAN_API_TOKEN` による Bearer / Cookie 認証（Worker 本番では Cloudflare Access を併用）。
- **バックアップ & S3 同期（Native）**: ローカル / WebDAV / S3 へのスケジュールバックアップ、および S3 互換ストレージへのアップロード専用メディア同期。

---

## 実行ランタイムの構成

| 項目 | Native | Cloudflare Worker |
| --- | --- | --- |
| サーバー / エントリ | `src/main.rs`（Axum + Tokio） | `worker/src/lib.rs`（Rust → Wasm） |
| データベース | SQLite（`{DATA_DIR}/Dantalian/db/dantalian.db`） | Cloudflare D1（`worker/migrations/`） |
| メディア保存先 | ローカルディスク（`images/`, `audio/`, `epubs/`） | Wasabi（S3 互換ストレージ・ブラウザ直送対応） |
| 音声変換処理 | アプリ内バックグラウンドワーカー | Queue + 外部コンテナ（`container/`, `audio_processor`） |
| 主な用途 | ローカル PC・自宅サーバー・LAN 内利用 | Cloudflare 上のサーバーレス運用 |

---

## クイックスタート（Native）

### 前提条件

- **Rust**: 1.96 以降（`Cargo.toml` は Rust edition 2024 を使用）
- **電子書籍リーダー（Bibi）を使う場合**: [Node.js](https://nodejs.org/)・[Bun](https://bun.sh/)（`bibi.lock.json` に固定された Bibi 成果物を `static/vendor/bibi/` へ取り込むため）

> `static/vendor/bibi/` は生成物であり Git 管理されていません。リーダー機能を利用する場合は、先に [`docs/BIBI_INTEGRATION.md`](docs/BIBI_INTEGRATION.md) の手順で Bibi をビルド・取り込みしてください（蔵書登録・音楽再生・API のみの起動であれば不要です）。

### ビルドと起動

```sh
# 必要に応じて環境変数ファイルを用意
cp .env.sample .env

# ビルド & 起動（デフォルト: http://127.0.0.1:3000）
cargo run --features native --bin dantalian
```

リリースビルドを作成する場合:

```sh
cargo build --release --features native --bin dantalian
```

> 依存クレートの完全なライセンス一覧を `/licenses/` に含めてビルドしたい場合は、`cargo-about`（`0.8.4`）を導入したうえで `DANTALIAN_GENERATE_LICENSES=1` を指定してビルドしてください（未指定時は `LICENSE` / `NOTICE` を表示する簡易ページが生成されます）。

### データディレクトリ構成

起動時に `DATA_DIR` 配下へ自動作成されます。

```text
{DATA_DIR}/
  Dantalian/
    db/
      dantalian.db    # SQLite データベース
    images/           # 表紙画像 (jpg, jpeg, png, webp, gif, avif)
    audio/            # 音声ファイル・省データ変換出力 (encoded/opus/, encoded/aac/)
    epubs/            # 書籍ファイル (epub, pdf, zip)
    uploads/          # チャンクアップロード一時領域
```

---

## 環境変数

Native は `.env` ファイル（[`dotenvy`](.env.sample)）または環境変数から設定を読み込みます。各項目の記入例は [`.env.sample`](.env.sample) を参照してください。

### 基本・ネットワーク・認証

| 変数名 | 説明 | デフォルト |
| --- | --- | --- |
| `DATA_DIR` | データ保存ルートディレクトリ | `~/Documents`（Windows: `C:\Users\{user}\Documents`） |
| `BIND_ADDR` | 待受 IP アドレス（ループバック以外は `DANTALIAN_API_TOKEN` 必須） | `127.0.0.1` |
| `PORT` | 待受ポート番号 | `3000` |
| `DANTALIAN_API_TOKEN` | API・メディア配信用トークン（Bearer / `dantalian_api_token` Cookie） | 未設定（ループバックのみ認証なし） |
| `RUST_LOG` | ログレベル（例: `dantalian=debug`） | `dantalian=info` |

> **セキュリティ注意**: Native サーバーは既定で `127.0.0.1` のみにバインドされます。LAN 等へ公開する場合は必ず `DANTALIAN_API_TOKEN` を設定したうえで `BIND_ADDR=0.0.0.0` 等を指定してください（トークン未設定のまま非ループバックを指定すると起動を拒否します）。

### 外部 API・ライセンス生成

| 変数名 | 説明 | デフォルト |
| --- | --- | --- |
| `DISCOGS_TOKEN` | Discogs Personal Access Token（DB 設定が優先） | 未設定 |
| `MUSICBRAINZ_CONTACT` | MusicBrainz 用 User-Agent 連絡先（DB 設定が優先） | `+https://github.com/Rumia-Channel/dantalian` |
| `OPEN_LIBRARY_CONTACT` | Open Library に送信する公開連絡先（Worker では Vars に設定） | 未設定 |
| `DANTALIAN_GENERATE_LICENSES` | ビルド時に `cargo-about` で全依存ライセンスページを生成 | 未設定（簡易ページ） |

### バックアップ・メディア同期（Native）

DB の `settings` テーブル（管理画面「管理 → 設定」）の値が環境変数より優先されます。

- **バックアップ**: `BACKUP_ENABLED`, `BACKUP_SCHEDULE_TIME`, `BACKUP_SCHEDULE_TZ`, `BACKUP_RETENTION`, `BACKUP_DEST_TYPE`（`local` / `webdav` / `s3`）, `BACKUP_LOCAL_PATH`, `BACKUP_WEBDAV_*`, `BACKUP_S3_*`
- **メディア同期（S3 upload-only）**: `MEDIA_SYNC_ENABLED`, `MEDIA_SYNC_TYPES`（既定 `epubs,audio`）, `MEDIA_SYNC_SCHEDULE_TIME`, `MEDIA_SYNC_SCHEDULE_TZ`, `MEDIA_SYNC_S3_*`（空の場合は `BACKUP_S3_*` を流用）

---

## 開発・テスト

```sh
# フォーマット・型チェック・Native テスト
cargo fmt --all -- --check
cargo check --no-default-features
cargo test --features native --all-targets

# Worker アセット生成・ビルド・テスト（worker/ ディレクトリで実行）
node worker/build_assets.mjs
cargo test --manifest-path worker/Cargo.toml
```

静的アセット（HTML / CSS / JS）を変更した際は、[`build.rs`](build.rs) のキャッシュバスタ（`ASSET_VERSION`）を更新するため Native / Worker のリビルドが必要です。開発ルールやリリース手順は [`AGENTS.md`](AGENTS.md) を参照してください。

---

## ドキュメント

| ドキュメント | 内容 |
| --- | --- |
| [`docs/BIBI_INTEGRATION.md`](docs/BIBI_INTEGRATION.md) | Bibi 2.3.6 のビルド・取り込み手順、プリセット、CSP、`Bibi.API`、シリーズナビゲーション仕様 |
| [`docs/EXTERNAL_APIS.md`](docs/EXTERNAL_APIS.md) | NDL サーチ・Web NDL Authorities・Open Library・Amazon・DLsite・narou.rs の連携仕様とレート制御 |
| [`docs/TECHNICAL_REFERENCE.md`](docs/TECHNICAL_REFERENCE.md) | Native / Worker 比較、アップロード上限・presigned multipart、S3 同期、省データ再生、API 更新契約、回帰テスト |
| [`docs/MIGRATION_RUNBOOK.md`](docs/MIGRATION_RUNBOOK.md) | `dantalian_migrate` を使った Native (SQLite) → Worker (D1 + Wasabi) 移行手順書 |
| [`docs/DEPLOYMENT_CHECKLIST.md`](docs/DEPLOYMENT_CHECKLIST.md) | Cloudflare Workers / D1 / Wasabi / Audio Processor の本番デプロイチェックリスト |
| [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md) | バイナリ・ソース配布時のライセンス遵守事項 |
| [`docs/FDK_AAC_RUST_HANDOFF.md`](docs/FDK_AAC_RUST_HANDOFF.md) | 純 Rust AAC エンコーダ（`fdk-aac-rust`）の移行・修正記録 |
| [`AGENTS.md`](AGENTS.md) | リポジトリ開発規約（依存追加・ファイル分割・コミット・リリース手順） |

---

## ライセンス・特許告知

本ソフトウェア本体は **BSD 2-Clause License**（[`LICENSE`](LICENSE)）の下で公開されています。

### FDK AAC ライセンス・特許告知

**2026-08-31:** 本プロジェクトは、`fdk-aac-rust` を通じて **Third-Party Modified Version of the Fraunhofer FDK AAC Codec Library for Android** を含みます。これは Fraunhofer の公式プロジェクトではなく、Fraunhofer の承認・推奨を受けていません。`fdk-aac-rust` は Fraunhofer FDK AAC の純 Rust 移植版（C/C++ コンパイラ不要）であり、Rust ポートへの変更およびこの告知の日付をここに明示します。

- 完全なライセンス本文は [`NOTICE`](NOTICE) に保持しています。配布物には [`NOTICE`](NOTICE) および [`MODULE_LICENSE_FRAUNHOFER`](MODULE_LICENSE_FRAUNHOFER) を含めてください。
- `NOTICE` は**特許ライセンスを付与しません**。AAC のエンコード・デコードを配布または利用する場合は、対象となる特許権者またはライセンス管理者から必要な許諾を取得する責任があります。
- Fraunhofer の名称を承認・推奨の表示に使用してはなりません。
- 配布時の確認事項は [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md) を参照してください。

### 外部データソースのクレジット

本アプリケーションは書誌情報の取得に [国立国会図書館サーチ API](https://ndlsearch.ndl.go.jp/help/api) および [Open Library API](https://openlibrary.org/developers/api)、著者典拠の取得に [Web NDL Authorities API](https://id.ndl.go.jp/information/sparql-11/) を利用しています。
