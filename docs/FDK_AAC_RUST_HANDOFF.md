# FDK AAC 依存関係移行記録

`fdk-aac` クレートの依存先決定と、それに至る調査の記録。

## 最終構成 (2026-08-31)

- **採用**: crates.io `fdk-aac-rust = "0"` (resolved 0.2.3)。Fraunhofer FDK AAC
  の純 Rust 移植版。C/C++ コンパイラ不要。
- `opus-rs = "0"` (resolved 0.1.32)。純 Rust Opus 実装。
- `src/audio_codec.rs`: `PureRustEncoderParameters` + `set_parameter()` 方式。
  `encode_transport_f32(&[f32])` でエンコード。入力は f32 インタリーブ。
  `ChannelMode` は u32 (1=mono, 2=stereo)。>2ch は明示エラー
  (デコード段で既に 2ch へクランプされるため実影響なし)。
- `EncoderParams` 構造体は不要。`input_samples_per_channel()` でフレーム長取得。
  `encoder_delay()` で遅延取得。EOF flush はサイレントフレームのエンコードで代替。

## 既知の制限: fdk-aac-rust 0.2.3 無音欠陥

純Rust AAC-LC エンコーダのビットストリーム書き出しに欠陥があり、
**標準デコーダ (ffmpeg) でのデコード結果が完全無音**。
解析フィルタバンク・量子化器は正常動作し非ゼロ係数を出すが、
スケーリング/global_gain セマンティクスが壊れている。

- クレート同梱デコーダでは rms≈0.65 / peak=1.0 の飽和音となり、
  エンコーダとデコーダが同じ壊れた規約を共有している。
- モノラルは帯域幅未指定のままだと `BitReservoirUnderflow` パニック
  (`set_bandwidth()` での回避が必要だが、無音は解消されない)。
- 同梱 CI は純Rustエンコードの可聴性ゲートを持たないため無音のままリリースされている。
- 再現ハーネス: `tools/fdk-aac-rust-probe/`(upstream 修正の検証に再利用可能)

**ADTS ビットストリーム自体は構造的に正当**であり、`0xfff1` 同期ワード、
sampling_frequency_index などのヘッダフィールドは正しい。ffmpeg による
デコードもエラーなく成功するが、デコード結果の PCM は無音 (peak ≈ 0.0)。

## 解消 (2026-09-06): Rumia-Channel/fdk-aac-rust fix/aac-lc-audibility

- 直した点: forward MDCT の正規直交化を除去 (ISO 非正規 domain)、スケールファクタ探索を
  `2^((S-64)/4)` wire domain で粗→細に走査 (8191 clamp が細側の単調性を壊すため)、
  不成立 band は無音 fallback、CBR は推定ではなく実書込バイトでリトライ
  (`BitReservoirUnderflow` 消滅)。
- 検証: 同梱 983 tests green、ffmpeg 復号で tone/music/chord 0.98+、
- 実曲 192k で ffmpeg 製 AAC と帯域別に ±0.02 で並ぶ。
- Dantalian は `Rumia-Channel/fdk-aac-rust rev f5d8ed5` を git 依存で使用。
- `aac_ffmpeg_roundtrip_retains_tone` が peak/RMS/ZCR の可聴性ゲートになった。

## 却下した選択肢と理由

### crates.io `fdk-aac` 0.8 (実 Fraunhofer FDK C コード)

- 実 FDK C コードを `fdk-aac-sys` 0.5 経由で cc ビルド。
- Windows MSVC でのビルド・テストは可能だが、C/C++ コンパイラとビルド環境が必須。
- ユーザーが純 Rust 依存を希望したため却下。

### Rumia-Channel/fdk-aac-rs.git (旧依存、実FDKバインディング 0.9.0)

- **リポジトリ自体が削除済み (HTTP 404)**。ロックされていた rev `fead732a...`
  は取得不能。ローカル cargo キャッシュにもオブジェクトなし
  (残骸は別プロジェクト shiguredo_fdk_aac のもの)。

### fdk-aac-rust の ffi feature

- `qmf_test_wrapper.cpp`(差分テスト用ブリッジ)が MSVC でコンパイル不可
  (C2664/C2668)。upstream の ffi 差分テストは Linux 専用で Windows 未対応。

## 検証結果 (fdk-aac-rust 0.2.3)

- `cargo build`: 成功 (C/C++ コンパイラ不要)
- `cargo test --lib audio_codec`: 構造的テストは合格
  (`aac_ffmpeg_roundtrip_retains_tone`: ADTS ヘッダ正当性、ffmpeg デコード
  成功を確認。peak/corr/SNR は無音のため検証対象外)

## API 差分メモ (fdk-aac 0.8 → fdk-aac-rust 0.2.3)

| 項目 | fdk-aac 0.8 | fdk-aac-rust 0.2.3 |
|---|---|---|
| 入力型 | `&[i16]` PCM | `&[f32]` インタリーブ |
| パラメータ | `EncoderParams { bit_rate, ... }` 構造体 | `PureRustEncoderParameters::new()` + `set_parameter()` |
| ChannelMode | `ChannelMode::Mono/Stereo` enum | `u32` (1=mono, 2=stereo) |
| フレーム長取得 | `encoder.info().frameLength` | `encoder.input_samples_per_channel()` |
| 遅延取得 | `encoder.info().nDelay` | `encoder.encoder_delay()` |
| エンコード | `encoder.encode(input, &mut buf)` → `EncodeInfo.output_size` | `encoder.encode_transport_f32(input)` → `Vec<u8>` |
| EOF flush | `encoder.encode(&[], &mut buf)` を出力空まで | サイレントフレームのエンコードで代替 (flush メソッドなし) |
| ビルド依存 | C/C++ コンパイラ (cc) | 不要 (純 Rust) |
