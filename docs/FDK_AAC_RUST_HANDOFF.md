# FDK AAC 依存関係移行記録 (2026-08-23)

`fdk-aac` クレートの依存先決定と、それに至る調査の記録。

## 最終構成

- **採用**: crates.io `fdk-aac = "0.8"`(実 Fraunhofer FDK C コードを
  `fdk-aac-sys` 0.5 経由で cc ビルド。Windows MSVC でのビルド・テスト確認済み)
- `src/audio_codec.rs`: `EncoderParams { ... }` 構造体初期化 + EOF drain
  (`encode(&[], ..)` を出力が空になるまで繰り返す)方式。
  `ChannelMode` は Mono/Stereo のみのため、>2ch は明示エラー
  (デコード段で既に 2ch へクランプされるため実影響なし)。

## 却下した選択肢と理由

### fdk-aac-rust (penguin425/fdk-aac-rust fork, v0.2.3) 純Rustエンコーダ

- 純Rust AAC-LC エンコーダのビットストリーム書き出しに欠陥があり、
  **標準デコーダ (ffmpeg) でのデコード結果が完全無音**。
  解析フィルタバンク・量子化器は正常動作し非ゼロ係数を出すが、
  スケーリング/global_gain セマンティクスが壊れている。
- クレート同梱デコーダでは rms≈0.65 / peak=1.0 の飽和音となり、
  エンコーダとデコーダが同じ壊れた規約を共有している。
- モノラルは帯域幅未指定のままだと `BitReservoirUnderflow` パニック
  (`set_bandwidth()` での回避が必要だが、無音は解消されない)。
- 同梱 CI は純Rustエンコードの可聴性ゲートを持たないため無音のままリリースされている。
- 再現ハーネス: `tools/fdk-aac-rust-probe/`(upstream 修正の検証に再利用可能)

### Rumia-Channel/fdk-aac-rs.git(旧依存、実FDKバインディング 0.9.0)

- **リポジトリ自体が削除済み (HTTP 404)**。ロックされていた rev `fead732a...`
  は取得不能。ローカル cargo キャッシュにもオブジェクトなし
  (残骸は別プロジェクト shiguredo_fdk_aac のもの)。

### fdk-aac-rust の ffi feature

- `qmf_test_wrapper.cpp`(差分テスト用ブリッジ)が MSVC でコンパイル不可
  (C2664/C2668)。upstream の ffi 差分テストは Linux 専用で Windows 未対応。

## 検証結果 (crates.io fdk-aac 0.8)

- `cargo test --lib audio_codec`: **11/11 合格**
  (`aac_ffmpeg_roundtrip_retains_tone`: 復号 peak ≈0.5 を確認)
- ユーザー提供 FLAC (119.53s, 44.1kHz stereo) をアプリ経路で変換:
  - 変換時間 0.68s (release)
  - デコード長 119.58s(AAC-LC プライミング分 +0.05s は妥当)
  - Peak +0.11 dBFS / RMS −18.7 dB = 正常な音楽レベル
    (旧純Rustパスでは Peak −113.8 dBFS の無音だった)

## 移行時の API 差分メモ (0.9.0 fork → 0.8.0 crates.io)

| 項目 | 旧 fork | crates.io 0.8 |
|---|---|---|
| `EncoderParams` | `new(bit_rate, rate, transport, mode, aot)` | 構造体リテラル(`bit_rate` 他) |
| `params.bandwidth` | `Some(hz)` 設定可 | フィールドなし(FDK 自動) |
| `ChannelMode` | Mode1_2 等のマルチチャンネル版あり | Mono/Stereo のみ |
| EOF drain | `encoder.flush(&mut buf)` | `encode(&[], &mut buf)` を出力空まで |
| `InfoStruct` | frameLength/maxOutBufBytes/nDelay | 同一(C構造体そのまま) |
