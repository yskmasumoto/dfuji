//! # dfuji-app
//!
//! ダイヤモンド富士計算のメインアプリケーションクレート
//!
//! ## 主な機能
//! - ダイヤモンド富士の観測可能性判定
//! - 単一地点での観測判定 (`point`)
//! - 緯度経度範囲での候補地点探索 (`range`)

pub mod app;
pub(crate) mod tools;

pub use app::{Alignment, RangeMatch, point, polygon, range};
