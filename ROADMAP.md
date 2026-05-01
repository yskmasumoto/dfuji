# ROADMAP

dfuji プロジェクトの今後の対応方針・既知の課題・改善候補を整理する。完了したものは [CHANGELOG.md](./CHANGELOG.md) に移す運用。

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

---

## 完了済み

CHANGELOG の `[Unreleased]` セクション、および各リリースの履歴を参照。

- 0.1.0-beta.2: polygon 精度改善（凸包 → envelope リング、二分法収束厳格化）
- Unreleased: polygon 頂点削減（envelope リング → 方位ビン集約、~1500 → ~180 頂点）
- Unreleased: BUG-001 修正（`estimate_center_az_from_fuji_for_time` の固定点反復が球面測地補正を逆方向に積み上げ、富士山から遠い東方向の polygon が縮退する症状）
