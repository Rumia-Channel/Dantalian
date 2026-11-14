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

## 生成物の静的アセット

- `static/vendor/bibi/`（Bibi 配布物）と `static/wasm/`（ブラウザ音声プリプロセッサ）は **生成物であり Git 管理しない**。`git status` に現れてもコミットしないこと。
- `static/wasm/` は `node scripts/build_audio_preprocessor.mjs` で生成する。ブラウザ側プリプロセッサは `isWorkerRuntime()` でガードされており（`static/js/edit/tracks.js`）、**Worker ランタイム専用**である。Native の `cargo build` では生成も実行もされず、ローカル Native 開発でこのファイルが無くても問題ない。生成には `Cargo.lock` の `wasm-bindgen` クレートと同じバージョンの `wasm-bindgen-cli` が必要（現行 0.2.127。`cargo install wasm-bindgen-cli --version 0.2.127 --locked`）。バージョンが違うと生成が失敗する。
- `worker/build_assets.mjs` は両方を再生成・検証してから `worker/public` に配置するため、Worker 側の作業では同スクリプトが先に実行される。

## DB スキーマ変更ルール

- バージョン 1.0.0 未満の場合、DB の構造が大きく変わる際は後方互換性を気にせず大幅な変更を加えてよい。
- バージョン 1.0.0 以降では、DB スキーマの大幅な変更（既存データの破棄を伴うもの）は加えてはならない。

## リリース手順

- バージョンは `python scripts/bump_version.py X.Y.Z` で一括更新すること。手で個別に書き換えないこと。
  - 更新対象: `Cargo.toml`、`worker/Cargo.toml`（package version と `dantalian` 依存の version）、`about.hbs`（ライセンスページの `Dantalian X.Y.Z` 表記）、`Cargo.lock` / `worker/Cargo.lock`（`cargo check` で再生成）
- バージョンバンプは `develop` でコミットし、`master` へ `git merge --no-ff develop` で合流させること（fast-forward 禁止）。
- タグは `master` 上でアノテーテッドタグとして切る: `git tag -a vX.Y.Z -m "Dantalian vX.Y.Z: <変更概要>"`
- `git push origin master vX.Y.Z develop` で master・タグ・develop をまとめてプッシュする。タグプッシュが本番デプロイ CI のトリガー。
- タグを切る前に `git grep "旧バージョン" -- '*.toml' '*.hbs'` で残存バージョン表記がないか確認すること。

## 機能単位のコミット

- 作業は小さく自己完結した機能単位に分け、動作確認できた区切りごとに細かく commit すること。全作業が終わるまで未コミット差分を溜めないこと。
- ユーザーから commit の明示指示がなくても、検証済みの変更は commit まで行うこと。
- push・タグ作成はユーザーの指示に従うこと。push が依頼されている作業では、各機能の commit 後にこまめに push すること。
- 設計変更や方針転換の前に完了分を commit し、必要ならその境界へすぐ戻せる状態にすること。
- 未検証・動作しない途中差分や、無関係なユーザーの変更を混ぜて commit しないこと。

## コミット前フォーマット規約

- コミット（および PR 作成）前は、必ずリポジトリ全体に対して `cargo fmt` を実行すること。
- こまめに `cargo fmt` をかけたり戻したりすると、不要な commit が増えるため避ける。
- 自動 pre-commit hook として `.githooks/pre-commit` が用意されている場合は、それを活用してよい（ローカルで `git config core.hooksPath .githooks` 設定済みであることが望ましい）。
- pre-commit hook は `cargo fmt --all` を実行し、その前後で unstaged diff (`git diff --`) を比較する。**cargo fmt によって working tree の unstaged diff が変わった場合のみ abort** し、ユーザに整形済み差分を確認させてから `git add` して再 commit させる運用である（自動 `git add` はしない）。ユーザがもともと持っていた unrelated な unstaged 変更は、cargo fmt で書き換えられなければそのまま通る。
