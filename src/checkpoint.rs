//! Read-only platform checkpoint presentation, independent of roads and route planning.
//!
//! Platform coordinates are BD-09 (or the existing GCJ-02 fallback). The map uses
//! WGS84, just like OSM, so conversion happens once when the view model is built.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheState {
    Fresh,
    Stale,
    Missing,
    Corrupted,
    Loading,
    Error,
}

impl CacheState {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fresh => "有效",
            Self::Stale => "已过期（仅供查看）",
            Self::Missing => "无缓存",
            Self::Corrupted => "缓存损坏",
            Self::Loading => "加载中",
            Self::Error => "加载失败",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crs {
    Bd09,
    Gcj02,
    Wgs84,
    Unknown,
}

impl Crs {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bd09 => "BD-09",
            Self::Gcj02 => "GCJ-02",
            Self::Wgs84 => "WGS84",
            Self::Unknown => "未知",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointSource {
    PlatformCache,
    PlatformApi,
}

impl CheckpointSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::PlatformCache => "平台缓存",
            Self::PlatformApi => "平台接口",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationStatus {
    Valid,
    Invalid(String),
}

#[derive(Clone, Debug)]
pub struct CheckpointViewModel {
    /// Local UI identity, never presented as an original platform identifier.
    pub internal_id: String,
    pub platform_id: Option<Value>,
    /// Original name, including an explicit empty string if the platform sent one.
    pub name: Option<String>,
    pub raw_latitude: Option<f64>,
    pub raw_longitude: Option<f64>,
    pub raw_crs: Crs,
    pub map_crs: Crs,
    pub map_latitude: Option<f64>,
    pub map_longitude: Option<f64>,
    pub source: CheckpointSource,
    pub cached_at: Option<i64>,
    pub validation_status: ValidationStatus,
    /// Complete platform row; unrecognized metadata is not discarded.
    pub raw: Value,
}

impl CheckpointViewModel {
    pub fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("未命名打卡点")
    }

    pub fn map_coordinate(&self) -> Option<(f64, f64)> {
        if self.validation_status != ValidationStatus::Valid {
            return None;
        }
        Some((self.map_latitude?, self.map_longitude?))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckpointStats {
    pub raw: usize,
    pub parsed: usize,
    pub valid: usize,
    pub invalid: usize,
    /// Additional rows with an already-seen original identifier; rows stay visible.
    pub duplicate_ids: usize,
    /// Additional valid rows at the same coordinate; different identities stay distinct.
    pub overlapping_coordinates: usize,
    pub missing_ids: usize,
    pub missing_names: usize,
}

#[derive(Clone, Debug)]
pub struct CheckpointData {
    pub points: Vec<CheckpointViewModel>,
    pub cache_state: CacheState,
    pub error: Option<String>,
    pub cached_at: Option<i64>,
    pub stats: CheckpointStats,
}

impl Default for CheckpointData {
    fn default() -> Self {
        Self::from_points(
            Vec::new(),
            CheckpointSource::PlatformCache,
            None,
            CacheState::Missing,
        )
    }
}

impl CheckpointData {
    pub fn from_points(
        points: Vec<Value>,
        source: CheckpointSource,
        cached_at: Option<i64>,
        cache_state: CacheState,
    ) -> Self {
        let mut stats = CheckpointStats {
            raw: points.len(),
            ..Default::default()
        };
        let mut seen_ids = HashSet::new();
        let mut seen_coordinates = HashSet::new();
        let mut identities = std::collections::HashMap::<String, usize>::new();
        let points = points
            .into_iter()
            .map(|raw| {
                if raw.is_object() {
                    stats.parsed += 1;
                }
                let platform_id = ["pointId", "id", "pointCode", "point_id"]
                    .into_iter()
                    .find_map(|key| {
                        raw.get(key)
                            .filter(|id| {
                                id.is_number() || id.as_str().is_some_and(|s| !s.trim().is_empty())
                            })
                            .cloned()
                    });
                let name = ["pointName", "name", "point_name"]
                    .into_iter()
                    .find_map(|key| raw.get(key).and_then(Value::as_str).map(str::to_owned));
                if platform_id.is_none() {
                    stats.missing_ids += 1;
                }
                if name.as_deref().is_none_or(|n| n.trim().is_empty()) {
                    stats.missing_names += 1;
                }
                let identity = platform_id
                    .as_ref()
                    .map(|id| {
                        let key = id.to_string();
                        if !seen_ids.insert(key.clone()) {
                            stats.duplicate_ids += 1;
                        }
                        format!("id:{key}")
                    })
                    .unwrap_or_else(|| format!("row:{raw}"));
                let hash = format!("{:x}", Sha256::digest(identity.as_bytes()));
                let occurrence = identities.entry(hash.clone()).or_default();
                let internal_id = format!("checkpoint-{}-{}", &hash[..20], *occurrence);
                *occurrence += 1;

                let bd = (
                    number(&raw, &["lat", "latitude"]),
                    number(&raw, &["lon", "lng", "longitude"]),
                );
                let gcj = (number(&raw, &["glat"]), number(&raw, &["glon", "glng"]));
                let (raw_latitude, raw_longitude, raw_crs) = if valid_pair(bd.0, bd.1) {
                    (bd.0, bd.1, Crs::Bd09)
                } else if valid_pair(gcj.0, gcj.1) {
                    (gcj.0, gcj.1, Crs::Gcj02)
                } else if bd.0.is_some() || bd.1.is_some() {
                    (bd.0, bd.1, Crs::Bd09)
                } else if gcj.0.is_some() || gcj.1.is_some() {
                    (gcj.0, gcj.1, Crs::Gcj02)
                } else {
                    (None, None, Crs::Unknown)
                };
                let coordinate = crate::api::fence::point_xy(&raw)
                    .map(|(lat, lon)| crate::api::school_roads::to_wgs(lat, lon))
                    .filter(|&(lat, lon)| valid_pair(Some(lat), Some(lon)));
                let (map_latitude, map_longitude, validation_status) = match coordinate {
                    Some((lat, lon)) => {
                        stats.valid += 1;
                        if !seen_coordinates.insert((lat.to_bits(), lon.to_bits())) {
                            stats.overlapping_coordinates += 1;
                        }
                        (Some(lat), Some(lon), ValidationStatus::Valid)
                    }
                    None => {
                        stats.invalid += 1;
                        let reason = if !raw.is_object() {
                            "点位不是 JSON 对象"
                        } else {
                            "坐标缺失、非有限值、超出范围或为 (0, 0)"
                        };
                        (None, None, ValidationStatus::Invalid(reason.into()))
                    }
                };
                CheckpointViewModel {
                    internal_id,
                    platform_id,
                    name,
                    raw_latitude,
                    raw_longitude,
                    raw_crs,
                    map_crs: Crs::Wgs84,
                    map_latitude,
                    map_longitude,
                    source,
                    cached_at,
                    validation_status,
                    raw,
                }
            })
            .collect();
        Self {
            points,
            cache_state,
            error: None,
            cached_at,
            stats,
        }
    }

    pub fn coordinates(&self) -> Vec<(f64, f64)> {
        self.points
            .iter()
            .filter_map(CheckpointViewModel::map_coordinate)
            .collect()
    }

    /// A failed refresh never discards known valid data or hides its actual freshness.
    pub fn retain_error(&mut self, error: String) {
        self.error = Some(error);
        if self.stats.valid == 0 {
            self.cache_state = CacheState::Error;
        }
    }
}

fn number(raw: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        raw.get(*key).and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.trim().parse().ok())
                .filter(|n: &f64| n.is_finite())
        })
    })
}

fn valid_pair(lat: Option<f64>, lon: Option<f64>) -> bool {
    match (lat, lon) {
        (Some(lat), Some(lon)) => {
            (lat != 0.0 || lon != 0.0) && crate::location::Coordinate::new(lat, lon, 0.0).is_ok()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn data(points: Vec<Value>) -> CheckpointData {
        CheckpointData::from_points(
            points,
            CheckpointSource::PlatformCache,
            Some(1000),
            CacheState::Fresh,
        )
    }

    #[test]
    fn preserves_original_metadata_and_uses_fallback_names_only_for_display() {
        let raw = json!({"pointId":"1001","pointName":"东区打卡点","lat":"28.2","lng":"112.9","extra":{"kept":true}});
        let d = data(vec![
            raw.clone(),
            json!({"lat":28.21,"lon":112.91}),
            json!({"id":7,"name":" ","lat":28.22,"lon":112.92}),
        ]);
        assert_eq!(d.points[0].raw, raw);
        assert_eq!(d.points[0].platform_id, Some(json!("1001")));
        assert_eq!(d.points[0].display_name(), "东区打卡点");
        assert_eq!(d.points[1].platform_id, None);
        assert_eq!(d.points[1].name, None);
        assert_eq!(d.points[1].display_name(), "未命名打卡点");
        assert_eq!(d.points[2].name.as_deref(), Some(" "));
        assert_eq!(d.points[2].platform_id, Some(json!(7)));
        assert_eq!(d.stats.missing_ids, 1);
        assert_eq!(d.stats.missing_names, 2);
        assert_eq!(d.points[0].cached_at, Some(1000));
        assert_eq!(d.points[0].source, CheckpointSource::PlatformCache);
    }

    #[test]
    fn every_raw_row_is_accounted_for_without_silent_coordinate_drops() {
        let d = data(vec![
            json!({"lat":28.2,"lon":112.9}),
            json!({"lat":91,"lon":112.9}),
            json!({"lat":0,"lon":0}),
            json!({"lat":"NaN","lon":112.9}),
            json!({"lat":28.2}),
            json!(null),
        ]);
        assert_eq!(d.stats.raw, 6);
        assert_eq!(d.stats.parsed, 5);
        assert_eq!(d.stats.valid, 1);
        assert_eq!(d.stats.invalid, 5);
        assert_eq!(d.points.len(), 6);
        assert_eq!(d.coordinates().len(), 1);
        assert_eq!(d.points[1].raw_latitude, Some(91.0));
        assert!(d.points[1..].iter().all(|p| p.map_coordinate().is_none()));
    }

    #[test]
    fn duplicate_ids_and_overlapping_coordinates_are_reported_without_deduplication() {
        let d = data(vec![
            json!({"id":1,"lat":28.2,"lon":112.9}),
            json!({"id":1,"lat":28.21,"lon":112.91}),
            json!({"id":2,"lat":28.2,"lon":112.9}),
        ]);
        assert_eq!(d.stats.duplicate_ids, 1);
        assert_eq!(d.stats.overlapping_coordinates, 1);
        assert_eq!(d.stats.valid, 3);
        assert_eq!(d.coordinates().len(), 3);
        let ids: HashSet<_> = d.points.iter().map(|p| &p.internal_id).collect();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn bd09_display_aligns_with_wgs84_osm_and_is_not_converted_twice() {
        for (lat, lon) in [(28.2, 112.9), (39.9, 116.4), (38.9, 121.5)] {
            let (bd_lat, bd_lon) = crate::track::geom::wgs84_to_bd09(lat, lon);
            let d = data(vec![json!({"lat":bd_lat,"lon":bd_lon})]);
            let p = &d.points[0];
            assert_eq!(p.raw_crs, Crs::Bd09);
            assert_eq!(p.raw_latitude, Some(bd_lat));
            let actual = p.map_coordinate().unwrap();
            assert!((actual.0 - lat).abs() < 1e-8, "{actual:?}");
            assert!((actual.1 - lon).abs() < 1e-8, "{actual:?}");
            assert_eq!(
                p.map_coordinate(),
                Some(actual),
                "reads must not convert again"
            );
        }
    }

    #[test]
    fn gcj_fallback_keeps_original_coordinate_system_and_matches_bd_parser() {
        let raw = json!({"lat":0,"lon":0,"glat":28.2,"glng":112.9});
        let d = data(vec![raw.clone()]);
        let p = &d.points[0];
        assert_eq!(p.raw_crs, Crs::Gcj02);
        assert_eq!((p.raw_latitude, p.raw_longitude), (Some(28.2), Some(112.9)));
        let bd = crate::api::fence::point_xy(&raw).unwrap();
        assert_eq!(
            p.map_coordinate(),
            Some(crate::api::school_roads::to_wgs(bd.0, bd.1))
        );
    }

    #[test]
    fn no_coordinate_swap_is_attempted_when_both_axes_are_in_range() {
        let d = data(vec![json!({"lat":45.0,"lon":28.0})]);
        assert_eq!(d.points[0].raw_latitude, Some(45.0));
        assert_eq!(d.points[0].raw_longitude, Some(28.0));
        assert_eq!(
            d.points[0].map_coordinate(),
            Some(crate::api::school_roads::to_wgs(45.0, 28.0))
        );
    }

    #[test]
    fn failed_refresh_preserves_valid_stale_data() {
        let mut d = data(vec![json!({"lat":28.2,"lon":112.9})]);
        d.cache_state = CacheState::Stale;
        let before = d.coordinates();
        d.retain_error("HTTP 502".into());
        assert_eq!(d.cache_state, CacheState::Stale);
        assert_eq!(d.coordinates(), before);
        assert_eq!(d.error.as_deref(), Some("HTTP 502"));
        let mut missing = CheckpointData::default();
        missing.retain_error("请求超时".into());
        assert_eq!(missing.cache_state, CacheState::Error);
    }
}

/// Reserved local editor model. It cannot be passed as a platform checkpoint or
/// serialized into platform API data by this display subsystem.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct UserWaypoint {
    pub local_id: String,
    pub name: Option<String>,
    pub wgs84_latitude: f64,
    pub wgs84_longitude: f64,
}
