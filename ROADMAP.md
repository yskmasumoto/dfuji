# ROADMAP

dfuji プロジェクトの今後の対応方針・既知の課題・改善候補を整理する。完了したものは [CHANGELOG.md](./CHANGELOG.md) に移す運用。

---

## 改修フェーズ

改善候補（IMPROVE-XXX）の対応順は影響範囲・依存関係・リスクに基づき以下のフェーズで進める方針。各フェーズは前フェーズ完了を前提とせず独立着手可能だが、原則として番号順に進めるとレビュー差分が清浄に保てる。

### Phase 1: 安全な掃除（独立・低リスク）

削除と公開境界の調整。**完了 (2026-05-14)**。対象: IMPROVE-009 / 010 / 011 / 016 / 017 / 018 / 023。

### Phase 2: 内部リファクタ（挙動不変）

リファクタリングのみで公開 API は不変。対象: IMPROVE-014（定数昇格）/ 015（`NaiveDateTime` ラッパ）/ 024（角度 util の geo 昇格）/ 003（`BTreeMap` 統合）/ 004（時間ループ形式統一）。

### Phase 3: API 設計判断

戻り値型・エラー方針の見直しを含むため設計判断が必要。対象: IMPROVE-007 + 013（連動、`calc_sunset_time` の戻り値変更）/ 019（エラー伝達方針）/ 020（`validate_lat_lon` を `Range` にも）/ 006（CHANGELOG 形式）/ 025（`.github/` 整理）。

### Phase 4: 物理ロジック検証・改善

物理現象を扱うため実測ベンチで前提を検証してから着手。対象: IMPROVE-008（時間ループ終端固定の検証、東方向取りこぼし）→ IMPROVE-001（観測者標高、破壊的変更）。

### Phase 5: 大規模分割

責務別ファイル分割。対象: IMPROVE-012（`tools.rs` / `app.rs` の責務別分割）。Phase 1-3 が終わった状態で切る方が責務境界が明確になる。

### テスト追加方針（IMPROVE-021 / 022 / 005）

- **IMPROVE-021**: Phase 2-3 でリファクタする関数のテストのみ先行追加（現状挙動を固定するゴールデンテストとして）。残りの `geo` / `sun` 関数群は Phase 5 前後でまとめて。
- **IMPROVE-022**: 削除分は Phase 1 で先行実施済み（IMPROVE-010 連動で `debug_polygon_point_count` 削除）。残り 2 件（`debug_known_hit_outside_polygon` / `diagnose_chiba_polygon_miss`）は Phase 5 のファイル分割時に `tests/` ディレクトリへ移すかを判断。
- **IMPROVE-005**: 360°/0° 跨ぎ回帰テスト。Phase 4 完了後（物理ロジックの基準値が安定してから）に着手。

---

## 既知バグ

現在オープンな既知バグはなし。修正済みの履歴は CHANGELOG.md を参照。

---

## 改善候補（バグではない）

### IMPROVE-001: 観測者標高の可変対応

現在、`tools.rs` の `geo::calc_altitude` 呼び出しは観測者標高を `0.0` 固定にしている。実地形では観測者が標高を持つ場合、地球曲率による富士山頂の地平線下没距離が伸びる（例: 筑波山 877m なら +112 km）。

**影響:** 200 km 以遠の山岳地（茨城北部・栃木・福島南部等）でダイヤモンド富士が見える物理的可能性があるが、現実装ではカバーできない。

**検討事項:**
- `point` / `range` の API シグネチャに `observer_altitude_m: f64` を追加（破壊的変更）
- もしくはオーバーロード関数を別名で提供
- `polygon` 用には DEM (Digital Elevation Model) の組み込みが必要だが、これは大幅な依存追加なのでオプション機能で別レイヤとする

### IMPROVE-003: `bin_times` と `bins` の二段階 BTreeMap 一本化

集約処理で `BTreeMap<i32, BTreeSet<NaiveDateTime>>` と `BTreeMap<i32, BinEdge>` を別々に構築している。`BinEntry { times, near, far }` 等に統合すれば構造がよりシンプルに。機能的には問題なし。

### IMPROVE-004: 時間ループの `for i in 0..=loop_n` 形式統一

`detect_alignment_for_location` は `for i in 0..=loop_n` 形式、`create_lonlat_vec` は `while offset_seconds <= end_offset_seconds` 形式。BUG-001 の修正と合わせて統一。

### IMPROVE-005: 360°/0° 跨ぎ日付の回帰テスト

現状の回帰テスト（`multiple_point_hit_locations_are_inside_polygon` 等）はいずれも方位帯が 360°/0° を跨がない日付（11 月）のみ。夏至前後など方位帯が跨ぎを起こしうる日付の回帰テストを追加して、anchor / `relative_az` / リング順序の不変条件を強化したい。

### IMPROVE-006: CHANGELOG 形式の整理

現在 `Notes` セクションを使っているが、Keep a Changelog 1.1.0 仕様外。`Changed` への統合か、設計判断は別ファイル（`ARCHITECTURE.md` 等）に分離するか検討。

### IMPROVE-007: `sun::calc_sunset_time` の日跨ぎ情報消失

`sun/src/lib.rs` の `calc_sunset_time` は `total_seconds.rem_euclid(SECONDS_PER_DAY)` で 0〜86399 秒に正規化した `(hour, minute, second)` を返すため、`tools.rs` の `normalize_sunset_naive_datetime` / `detect_alignment_for_location` が `div_euclid` で `day_offset` を復元しようとしても情報が既に失われている（`day_offset` は常に 0）。

実害は関東〜中部の通常日付では発生しない（日没は JST 16〜17 時台で日跨ぎが起きない）が、API 設計として戻り値を `i64`（UNIX 秒）または `Option<NaiveDateTime>` に変えれば呼び出し側が大幅に簡素化される。

### IMPROVE-008: 時間ループ終端が日没ぴったり固定で、東方面の観測ラインを取りこぼす可能性

`app/src/tools.rs` の `create_lonlat_vec` 内、時間ループの終端が `end_offset_seconds = 0`（富士山地点の日没ちょうど）固定になっている。

```rust
let start_offset_seconds = -(OBSERVATION_OFFSET_HOURS * 60 * 60);  // -7200
let end_offset_seconds = 0;                                         // 富士山地点の日没ちょうど
```

ダイヤモンド富士の観測時刻は「観測者から見て太陽が富士山頂と重なる瞬間」で、観測者位置によって富士山地点の日没時刻と数分〜数十分ズレる。富士山から遠い東方向では観測時刻が **富士山地点の日没より後ろにずれる** ため、ループ終端固定で取りこぼす可能性がある。

なお BUG-001（beta.4 で修正済み）の原因は `estimate_center_az_from_fuji_for_time` の固定点反復誤差であり、本項目とは別問題。

**検証の最短経路:**
1. `tools.rs` の `end_offset_seconds` を一時的に `30 * 60` に変更
2. `cargo build --release && ./target/release/dfuji-cli polygon --year 2026 --month 2 --day 24 > /tmp/check.geojson`
3. ダイヤモンド富士の観測可能エリアを公開している既存サービスをベンチマークとして用い、千葉以東までラインが伸びるかを目視比較

**修正方針候補:**
- 案 A: `OBSERVATION_AFTER_SUNSET_MINUTES` 定数追加で終端を `+N分` に拡張（最小変更、対称性は崩れる）
- 案 B: `OBSERVATION_WINDOW_BEFORE_HOURS` / `OBSERVATION_WINDOW_AFTER_HOURS` の対称ウィンドウ化（互換破壊）
- 案 C: 観測者地点ごとの現地日没時刻を時間ループ基点にする（物理的に最も正しい、構造変更大）

### IMPROVE-012: `app/src/tools.rs`(868行) / `app/src/app.rs`(852行) の責務別分割

`tools.rs` は sunset / 二分法 / 角度 util / detect / polygon の 5 責務が同居。`app.rs` はテストヘルパー `is_point_inside_polygon_geojson`（120 行）と `#[ignore]` テスト 3 件（約 500 行）で本体の 6 倍に膨張している。

### IMPROVE-013: `detect_alignment_for_location` 内の日没分解処理の重複解消

`tools.rs` L355-395 の日没分解は `normalize_sunset_naive_datetime` と完全同一ロジック。IMPROVE-007 解消と連動して `normalize_sunset_naive_datetime` 呼び出し 1 行に置換できる。

### IMPROVE-014: `SECONDS_PER_DAY` 定数の `dfuji-core` 昇格

現状 `sun/src/lib.rs:183` と `tools.rs:32` / L355 に重複定義。

### IMPROVE-015: `sun::calc_sun_az_and_alt` の `NaiveDateTime` 受け取りラッパ追加

`year as i16, month as u8, day as u8, hour as u8, minute as u8, second as f64` の 6 引数変換ボイラープレートが 5 か所に重複（`tools.rs:72-82, 176-186, 206-216, 430-440`, `app.rs:406-416, 536-546`）。

### IMPROVE-019: エラー伝達方針の整理

公開 API は全て `Option::None` + `eprintln!`。`thiserror` で構造化エラー型を導入するか、`Option` を維持するかを決める。

### IMPROVE-020: `cli::validate_lat_lon` を `Range` サブコマンドにも適用

現状は `Point` のみ。`Range` 入力は未バリデーション。

### IMPROVE-021: `geo` / `sun` のユニットテスト追加

`calc_altitude` / `calc_azimuth` / `calc_destination_point` / `geodetic_to_ecef` / `calc_sun_az_and_alt` / `calc_sunset_time` にテストがない。

### IMPROVE-022: `#[ignore]` デバッグテストの整理

修正済みバグの調査用テスト。`debug_polygon_point_count` は IMPROVE-010 連動で Phase 1 にて削除済み。残り 2 件（`debug_known_hit_outside_polygon` / `diagnose_chiba_polygon_miss`、合計 約 400 行）はリグレッション資料として残すか、`tests/` ディレクトリへ移すかを Phase 5 のファイル分割時に決定する。

### IMPROVE-024: `geo::angular_diff_deg` 等の角度ユーティリティの `geo` 昇格

現在 `app/src/tools.rs` 内 `pub(crate)` で `angular_diff_deg` / `angular_separation_deg` が定義されている。`geo` クレートに移すと再利用性が上がるが、`geo` の責務範囲を「方位/距離計算」から「角度数学全般」に広げる判断が要る。

### IMPROVE-025: `.github/` 配下の整理（次の CI 整備と合わせて実施）

- `.github/copilot-instructions.md` L8 に存在しない `./agent` ディレクトリへの言及
- `.github/copilot-instructions.md` / `rust.instructions.md` / `CLAUDE.md` の内容重複
- `.github/workflows/ci.yml:41` の「現在はテストコードがないかもしれませんが」テンプレ残骸コメント

---

## 完了済み

CHANGELOG の各リリースの履歴を参照。

- 0.1.0-beta.2: polygon 精度改善（凸包 → envelope リング、二分法収束厳格化）
- 0.1.0-beta.3: polygon 頂点削減（envelope リング → 方位ビン集約、~1500 → ~180 頂点）
- 0.1.0-beta.4: BUG-001 修正（`estimate_center_az_from_fuji_for_time` の固定点反復が球面測地補正を逆方向に積み上げ、富士山から遠い東方向の polygon が縮退する症状）
- 0.1.0-beta.5: `BISECTION_HIGH_DISTANCE` を 200 km → 250 km に拡張（IMPROVE-002）。銚子（富士山から ~195 km）の `d_far` が解けるように
- 0.1.0-beta.6: Phase 1（掃除と公開境界の調整）。死 API・残骸ディレクトリ・旧バイナリの削除、公開境界の縮小、`tracing`/`tracing-subscriber` の workspace 一元管理（IMPROVE-009/010/011/016/017/018/023）

### Phase 1 完了 (2026-05-14)

掃除と公開境界の調整。`0.1.0-beta.6` に含まれる。

- IMPROVE-009: 旧 `app/src/main.rs` と `app/Cargo.toml` の `tracing-subscriber` 依存を削除
- IMPROVE-010: `geo::solver_distance_for_altitude` / `bisection_method`（死 API）と、それを唯一参照していた `#[ignore]` テスト `debug_polygon_point_count` を削除
- IMPROVE-011: `wasm/` / `dfuji/target/` 残骸ディレクトリの物理削除（git 追跡外）
- IMPROVE-016: `sun::my_decimal_day` の `pub use` を撤回し `pub(crate)` 化。doc test を `helpers` モジュール内の unit test に変換
- IMPROVE-017: `app/src/lib.rs` の `pub mod tools;` を `pub(crate) mod tools;` に降格
- IMPROVE-018: `pub use dfuji_geo as geo; pub use dfuji_sun as sun;` の再公開を撤回（案 a：全撤回）
- IMPROVE-023: `tracing` / `tracing-subscriber` を `[workspace.dependencies]` で一元管理に変更
