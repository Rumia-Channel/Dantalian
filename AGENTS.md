# Dantalian - Agent Guidelines

## ライブラリ追加ルール

- 新しいライブラリ（クレート）を追加するときは、`Cargo.toml` に直接書き込まないこと。
- 必ず `cargo add <crate-name>` コマンドを使用して追加すること。
- バージョン指定が必要な場合は `cargo add <crate-name>@<version>` を使用すること。
- features を指定する場合は `cargo add <crate-name> --features <feature>` を使用すること。
- `Cargo.toml` の依存クレートのバージョンは、最上位のメジャーバージョンだけを指定すること。`0.1.26` は `"0"`、`1.2.3` は `"1"` とし、minor/patch 番号や exact pin（`=...`）は指定しないこと。
- 依存クレートのバージョンを更新した場合は、更新後の全依存バージョンで Native / Worker のビルドと関連テストを実行し、API 変更を互換修正すること。

## ファイル分割ルール

- WEB UI（HTML/CSS/JS）および Rust（src/）のファイルは、責任ごとに分割して一ファイル当たりの容量を減らすこと。
- 1ファイルが肥大化した場合は、機能単位で別ファイルに切り出すこと。
  - Rust: `mod` で分割し、`src/` 配下に配置（例: `api/books.rs`, `api/series.rs`）
  - CSS: 機能ごとに別ファイルを作成し、`index.html` で `<link>` 読み込み
  - JS: 機能ごとに別ファイルを作成し、`index.html` で `<script>` 読み込み
- 新しい静的アセットファイルを追加した場合は、`build.rs` のキャッシュバスタ設定も忘れずに更新すること。

## Web UI アセットのキャッシュバスタ

- 静的アセット（HTML/CSS/JS/フォント）の変更をブラウザに即座に反映させるため、キャッシュバスタ（`?v=<hash>`）を使用している。
- `build.rs` が全アセットファイルの更新日時をハッシュ化し、`ASSET_VERSION` 環境変数にセットする。
- 各 HTML 内の `?v=ASSET_VERSION` はコンパイル時に実際のハッシュ値に置換される（`src/main.rs` の `serve_html` ハンドラ経由）。
- CSS/JS を変更した場合は必ずリビルド（`cargo build`）すること。リビルドしないとハッシュが更新されず、ブラウザが古いキャッシュを使い続ける。
- 新しい静的アセット（CSS/JS ファイル）を追加した場合は、`build.rs` に `println!("cargo:rerun-if-changed=static/<file>");` を追加し、`index.html` にも `?v=ASSET_VERSION` を付与すること。

## DB スキーマ変更ルール

- バージョン 1.0.0 未満の場合、DB の構造が大きく変わる際は後方互換性を気にせず大幅な変更を加えてよい。
- バージョン 1.0.0 以降では、DB スキーマの大幅な変更（既存データの破棄を伴うもの）は加えてはならない。

## コミット前フォーマット規約

- コミット（および PR 作成）前は、必ずリポジトリ全体に対して `cargo fmt` を実行すること。
- こまめに `cargo fmt` をかけたり戻したりすると、不要な commit が増えるため避ける。
- 自動 pre-commit hook として `.githooks/pre-commit` が用意されている場合は、それを活用してよい（ローカルで `git config core.hooksPath .githooks` 設定済みであることが望ましい）。
- pre-commit hook は `cargo fmt --all` を実行し、その前後で unstaged diff (`git diff --`) を比較する。**cargo fmt によって working tree の unstaged diff が変わった場合のみ abort** し、ユーザに整形済み差分を確認させてから `git add` して再 commit させる運用である（自動 `git add` はしない）。ユーザがもともと持っていた unrelated な unstaged 変更は、cargo fmt で書き換えられなければそのまま通る。
