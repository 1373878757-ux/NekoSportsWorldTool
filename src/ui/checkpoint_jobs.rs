//! Read-only map metadata jobs. Never create route inputs from display-only stale data.
use super::{App, WakeSender};
use crate::api::{client::ApiClient, model};
use crate::checkpoint::{CacheState, CheckpointData, CheckpointSource};
use crate::location::Coordinate;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Scope = (i64, String, Option<Coordinate>, String);

pub struct CheckpointLayerState {
    pub data: CheckpointData,
    pub busy: bool,
    pub request_id: u64,
    pub selected: Option<String>,
    pub fit_all: bool,
    pub focus: bool,
    pub last_response_stats: Option<crate::checkpoint::CheckpointStats>,
    pub scope_note: String,
    scope: Option<Scope>,
    campus_selection: Option<String>,
    cancel: Arc<crate::task_control::Control>,
    attempts: Arc<Mutex<VecDeque<Instant>>>,
    commit_gate: Arc<Mutex<()>>,
}
impl Default for CheckpointLayerState {
    fn default() -> Self {
        Self {
            data: CheckpointData::default(),
            busy: false,
            request_id: 0,
            selected: None,
            fit_all: true,
            focus: false,
            scope: None,
            campus_selection: None,
            last_response_stats: None,
            scope_note: String::new(),
            cancel: Arc::new(crate::task_control::Control::default()),
            attempts: Arc::new(Mutex::new(VecDeque::new())),
            commit_gate: Arc::new(Mutex::new(())),
        }
    }
}

#[derive(Default)]
pub struct BoundaryLayerState {
    /// Platform boundary coordinates remain BD-09; convert once at the map boundary.
    pub fences: Vec<Vec<(f64, f64)>>,
    pub status: String,
}

#[derive(Clone)]
pub(super) struct LayerRequest {
    id: u64,
    epoch: u64,
    scope: Option<Scope>,
    cancel: Arc<crate::task_control::Control>,
    attempts: Arc<Mutex<VecDeque<Instant>>>,
    commit_gate: Arc<Mutex<()>>,
}

pub(super) enum LayerUpdate {
    Data(CheckpointData),
    ScopeMismatch,
    ScopeNote(String),
    Observed(CheckpointData),
    Error(String),
    Boundary {
        fences: Vec<Vec<(f64, f64)>>,
        status: String,
    },
    Finished,
}
pub(super) struct LayerLoad {
    pub request: LayerRequest,
    pub update: LayerUpdate,
}

fn freshness(snapshot: &model::PointsSnapshot) -> Result<CacheState, String> {
    Ok(
        match snapshot.freshness(crate::crypto::envelope::now_ms())? {
            model::PointsSnapshotFreshness::Fresh => CacheState::Fresh,
            model::PointsSnapshotFreshness::Stale => CacheState::Stale,
        },
    )
}

fn cached_display(request: &LayerRequest, area: Option<&Option<String>>) -> LayerUpdate {
    let data = match model::read_points_snapshot() {
        Ok(Some(snapshot)) => {
            if let Some((uid, school, anchor, _)) = &request.scope {
                if snapshot.scope.uid != *uid
                    || snapshot.scope.unid != *school
                    || !anchor.is_some_and(|p| snapshot.anchor.is_near(p, 0.0001))
                    || area.is_some_and(|a| snapshot.scope.run_area_id != *a)
                {
                    return LayerUpdate::ScopeMismatch;
                }
            }
            match freshness(&snapshot) {
                Ok(state) => CheckpointData::from_points(
                    snapshot.points,
                    CheckpointSource::PlatformCache,
                    Some(snapshot.ts),
                    state,
                ),
                Err(error) => corrupted(error),
            }
        }
        Ok(None) => CheckpointData::default(),
        Err(error) => corrupted(error),
    };
    LayerUpdate::Data(data)
}
fn corrupted(error: String) -> CheckpointData {
    CheckpointData {
        cache_state: CacheState::Corrupted,
        error: Some(error),
        ..Default::default()
    }
}
fn publish(sender: &WakeSender<LayerLoad>, request: &LayerRequest, update: LayerUpdate) {
    if request.cancel.check().is_ok() {
        let _ = sender.send(LayerLoad {
            request: request.clone(),
            update,
        });
    }
}

pub(super) struct MapMetadata {
    pub campuses: Vec<crate::api::campus::Campus>,
    pub selected_key: String,
    pub fences: Vec<Vec<(f64, f64)>>,
    pub points: Result<Vec<serde_json::Value>, String>,
}

/// Shared by independent point refresh and the existing road job. Points are published
/// before any road or policy operation. Authentication and HTTP stay in the existing API.
pub(super) fn load_metadata(
    client: &mut ApiClient,
    anchor: Option<Coordinate>,
    selection: Option<&crate::api::campus::Selection>,
    request: &LayerRequest,
    sender: &WakeSender<LayerLoad>,
    log: &mut dyn FnMut(&str),
) -> Result<MapMetadata, String> {
    publish(
        sender,
        request,
        LayerUpdate::ScopeNote("缓存区域尚未通过本次围栏响应核对；仅供查看".into()),
    );
    publish(sender, request, cached_display(request, None));
    let result = (|| {
        request.cancel.check()?;
        let response = match crate::api::fence::fetch_geo_fence_response(client) {
            Ok(value) => value,
            Err(error) => {
                publish(
                    sender,
                    request,
                    LayerUpdate::Boundary {
                        fences: Vec::new(),
                        status: format!("围栏刷新失败：{error}；保留已有边界"),
                    },
                );
                return Err(error);
            }
        };
        request.cancel.check()?;
        let campuses = crate::api::campus::catalog(&response);
        let school = client.login.as_ref().ok_or("未登录")?.unid.clone();
        let campus = crate::api::campus::chosen(&campuses, selection, &school)
            .or_else(|| {
                campuses.iter().find(|c| {
                    anchor.is_some_and(|anchor| {
                        c.polygons.iter().any(|p| {
                            route_planner::point_in_polygon(
                                &p.iter()
                                    .map(|&(a, b)| route_planner::Coord::new(b, a))
                                    .collect::<Vec<_>>(),
                                route_planner::Coord::new(anchor.longitude, anchor.latitude),
                            )
                        })
                    })
                })
            })
            .or_else(|| campuses.first());
        let fences = campus.map(|c| c.polygons.clone()).unwrap_or_default();
        publish(
            sender,
            request,
            LayerUpdate::Boundary {
                fences: fences.clone(),
                status: if fences.is_empty() {
                    "未提供校园围栏".into()
                } else {
                    format!("已加载 {} 个围栏", fences.len())
                },
            },
        );
        let area = if let Some(campus) = campus {
            campus.run_area_id(response.area.run_area_id)?
        } else {
            (response.area.run_area_id >= 0).then(|| response.area.run_area_id.to_string())
        };
        // Do not keep checkpoints belonging to a different server-selected area.
        publish(sender, request, cached_display(request, Some(&area)));
        publish(
            sender,
            request,
            LayerUpdate::ScopeNote("当前请求的学校区域已确认；缓存仅在范围匹配时显示".into()),
        );
        let points: Result<Vec<serde_json::Value>, String> = (|| {
            let anchor = anchor.ok_or("请配置定位锚点；已有缓存仍可查看")?;
            request.cancel.check()?;
            let scope = model::PointsCacheScope {
                uid: client.login.as_ref().ok_or("未登录")?.uid,
                unid: school.clone(),
                run_area_id: area.clone(),
            };
            let cached = model::load_points_cache_for(anchor, &scope);
            if cached.is_none() {
                let mut attempts = request
                    .attempts
                    .lock()
                    .map_err(|_| "点位刷新限流状态不可用")?;
                let now = Instant::now();
                while attempts
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(300))
                {
                    attempts.pop_front();
                }
                if attempts.len() >= 3 {
                    return Err(
                        "点位刷新受限：300 秒内最多 3 次未命中缓存的请求，请稍后再试".into(),
                    );
                }
                attempts.push_back(now);
            }
            let source = if cached.is_some() {
                CheckpointSource::PlatformCache
            } else {
                CheckpointSource::PlatformApi
            };
            let mut observe = |rows: &[serde_json::Value]| {
                if source != CheckpointSource::PlatformApi {
                    return;
                }
                publish(
                    sender,
                    request,
                    LayerUpdate::Observed(CheckpointData::from_points(
                        rows.to_vec(),
                        source,
                        cached.as_ref().map(|(ts, _)| *ts),
                        CacheState::Fresh,
                    )),
                );
            };
            let mut save_error = None;
            let mut persist =
                |anchor, scope: &model::PointsCacheScope, values: &[serde_json::Value]| {
                    let _guard = request
                        .commit_gate
                        .lock()
                        .map_err(|_| "点位缓存提交锁不可用")?;
                    request.cancel.check()?;
                    if let Err(error) = model::save_points_cache(anchor, scope, values) {
                        save_error = Some(format!("点位已获取，但缓存保存失败：{error}"));
                    }
                    Ok(())
                };
            let values = crate::api::points::fetch_points_ext_observed(
                client,
                anchor,
                area,
                log,
                &mut observe,
                &mut persist,
            )?;
            request.cancel.check()?;
            let saved_ts = model::read_points_snapshot()
                .ok()
                .flatten()
                .filter(|snapshot| snapshot.scope == scope && snapshot.points == values)
                .map(|snapshot| snapshot.ts);
            publish(
                sender,
                request,
                LayerUpdate::Data(CheckpointData::from_points(
                    values.clone(),
                    source,
                    saved_ts,
                    CacheState::Fresh,
                )),
            );
            if let Some(error) = save_error {
                publish(sender, request, LayerUpdate::Error(error));
            }
            Ok(values)
        })();
        if let Err(error) = &points {
            publish(sender, request, LayerUpdate::Error(error.clone()));
        }
        Ok(MapMetadata {
            selected_key: campus.map(|c| c.key.clone()).unwrap_or_default(),
            campuses,
            fences,
            points,
        })
    })();
    if let Err(error) = &result {
        publish(sender, request, LayerUpdate::Error(error.clone()));
    }
    // Loading reflects only metadata, never the road request that may follow it.
    publish(sender, request, LayerUpdate::Finished);
    result
}

impl App {
    /// A road response may infer the same campus whose metadata was just loaded.
    /// Adopt that UI selection without retiring this request's point layer. An
    /// actual user selection change still invalidates the layer on the next poll.
    pub(super) fn adopt_checkpoint_campus(&mut self, campus: &crate::api::campus::Selection) {
        let previous = self
            .config
            .school_campus
            .as_ref()
            .map(|value| format!("{}:{}", value.school, value.key));
        if self.checkpoints.scope.is_some() && self.checkpoints.campus_selection == previous {
            self.checkpoints.campus_selection = Some(format!("{}:{}", campus.school, campus.key));
        }
    }

    pub(super) fn clear_checkpoint_scope(&mut self) {
        self.run_page.checkpoint_fitted = false;
        self.osm_page.fitted = false;
        self.cancel_checkpoint_load();
        self.checkpoints = CheckpointLayerState {
            request_id: self.checkpoints.request_id,
            attempts: self.checkpoints.attempts.clone(),
            ..Default::default()
        };
        self.boundary = BoundaryLayerState::default();
    }

    pub(crate) fn cancel_checkpoint_load(&mut self) {
        // Serialize cancellation with the guarded cache commit of this layer.
        let _guard = self
            .checkpoints
            .commit_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.checkpoints.cancel.cancel();
        self.checkpoints.request_id = self.checkpoints.request_id.wrapping_add(1);
        self.checkpoints.busy = false;
        if self.checkpoints.data.cache_state == CacheState::Loading {
            self.checkpoints.data.cache_state = CacheState::Missing;
        }
    }

    pub(super) fn begin_checkpoint_load(&mut self) -> LayerRequest {
        self.cancel_checkpoint_load();
        let campus_selection = self
            .config
            .school_campus
            .as_ref()
            .map(|campus| format!("{}:{}", campus.school, campus.key));
        let scope = if crate::runtime::offline() {
            None
        } else {
            self.session.as_ref().map(|s| {
                (
                    s.uid,
                    s.unid.clone(),
                    self.identity.anchor_coordinate().ok(),
                    s.token.clone(),
                )
            })
        };
        if scope != self.checkpoints.scope || campus_selection != self.checkpoints.campus_selection
        {
            self.checkpoints.data = CheckpointData::default();
            self.checkpoints.selected = None;
            self.boundary = BoundaryLayerState::default();
        }
        self.checkpoints.last_response_stats = None;
        self.checkpoints.scope_note = if crate::runtime::offline() {
            "离线：仅显示当前数据目录的缓存；未在线核对区域".into()
        } else {
            "正在核对缓存范围".into()
        };
        self.checkpoints.scope = scope.clone();
        self.checkpoints.campus_selection = campus_selection;
        self.checkpoints.busy = true;
        if self.checkpoints.data.stats.valid == 0 {
            self.checkpoints.data.cache_state = CacheState::Loading;
        }
        self.checkpoints.cancel = Arc::new(crate::task_control::Control::default());
        LayerRequest {
            id: self.checkpoints.request_id,
            epoch: self.session_epoch,
            scope,
            cancel: self.checkpoints.cancel.clone(),
            attempts: self.checkpoints.attempts.clone(),
            commit_gate: self.checkpoints.commit_gate.clone(),
        }
    }

    pub(crate) fn refresh_checkpoint_layer(&mut self) {
        if self.checkpoints.busy || (!crate::runtime::offline() && self.osm_page.busy) {
            return;
        }
        if !crate::runtime::offline() && self.session.is_none() {
            self.checkpoints
                .data
                .retain_error("请先登录后刷新平台打卡点".into());
            return;
        }
        let offline = crate::runtime::offline();
        let request = self.begin_checkpoint_load();
        let sender = self.layer_tx.clone();
        let identity = self.identity.clone();
        let session = self.session.clone();
        let selection = self.config.school_campus.clone();
        #[cfg(test)]
        let fixture = self.route_fixture.clone();
        self.spawn_session_job(move |tx| {
            #[cfg(test)]
            if let Some((_, dir)) = &fixture {
                crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(dir.clone()));
            }
            if offline {
                publish(&sender, &request, cached_display(&request, None));
                publish(&sender, &request, LayerUpdate::Finished);
                return;
            }
            let anchor = identity
                .anchor_coordinate()
                .ok()
                .filter(|_| !identity.has_unconfigured_default_location());
            let mut client = tx.client(identity, session);
            #[cfg(test)]
            if let Some((host, _)) = fixture {
                client.set_test_host(host);
            }
            let mut log = App::logger(tx);
            let _ = load_metadata(
                &mut client,
                anchor,
                selection.as_ref(),
                &request,
                &sender,
                &mut log,
            );
        });
    }

    pub(super) fn poll_checkpoint_results(&mut self) {
        if self.checkpoints.data.cache_state == CacheState::Fresh {
            if let Some(ts) = self.checkpoints.data.cached_at {
                let remaining = ts
                    .saturating_add(model::POINTS_TTL_MS)
                    .saturating_sub(crate::crypto::envelope::now_ms());
                if remaining <= 0 {
                    self.checkpoints.data.cache_state = CacheState::Stale;
                } else {
                    self.ctx
                        .request_repaint_after(Duration::from_millis(remaining as u64));
                }
            }
        }
        // Retire stale school/anchor data even if no worker result arrives.
        if !crate::runtime::offline()
            && self.checkpoints.scope.as_ref().is_some_and(|scope| {
                self.session
                    .as_ref()
                    .is_none_or(|s| s.uid != scope.0 || s.unid != scope.1 || s.token != scope.3)
                    || self.identity.anchor_coordinate().ok() != scope.2
                    || self
                        .config
                        .school_campus
                        .as_ref()
                        .map(|campus| format!("{}:{}", campus.school, campus.key))
                        != self.checkpoints.campus_selection
            })
        {
            self.clear_checkpoint_scope();
        }
        while let Ok(load) = self.layer_rx.try_recv() {
            let request = &load.request;
            if request.id != self.checkpoints.request_id
                || request.epoch != self.session_epoch
                || request.scope != self.checkpoints.scope
                || request.cancel.check().is_err()
            {
                continue;
            }
            match load.update {
                LayerUpdate::Observed(data) => {
                    self.checkpoints.last_response_stats = Some(data.stats.clone());
                    if data.stats.invalid > 0 {
                        if self.checkpoints.data.stats.valid == 0 {
                            self.checkpoints.data = data;
                            self.checkpoints.data.cache_state = CacheState::Error;
                        }
                        self.checkpoints.data.retain_error(
                            "接口含无效坐标，未覆盖合法缓存；详情统计保留本次响应数量".into(),
                        );
                    }
                }
                LayerUpdate::ScopeNote(note) => self.checkpoints.scope_note = note,
                LayerUpdate::ScopeMismatch => {
                    self.checkpoints.data = CheckpointData::default();
                    self.checkpoints.selected = None;
                    self.checkpoints.scope_note =
                        "缓存与当前账号、学校、区域或锚点不匹配；未显示旧范围数据".into();
                }
                LayerUpdate::Data(data) => {
                    if matches!(
                        data.cache_state,
                        CacheState::Corrupted | CacheState::Missing
                    ) && self.checkpoints.data.stats.valid > 0
                    {
                        self.checkpoints.data.cache_state = data.cache_state;
                        self.checkpoints.data.error = data
                            .error
                            .or_else(|| Some("磁盘缓存缺失；保留当前内存点位，仅供查看".into()));
                        continue;
                    }
                    let changed = self.checkpoints.data.coordinates() != data.coordinates();
                    self.checkpoints.data = data;
                    if self.checkpoints.selected.as_ref().is_some_and(|id| {
                        !self
                            .checkpoints
                            .data
                            .points
                            .iter()
                            .any(|point| &point.internal_id == id)
                    }) {
                        self.checkpoints.selected = None;
                    }
                    if changed {
                        self.checkpoints.fit_all = true;
                        self.osm_page.fitted = false;
                        self.run_page.checkpoint_fitted = false;
                    }
                }
                LayerUpdate::Error(error) => self.checkpoints.data.retain_error(error),
                LayerUpdate::Boundary { fences, status } => {
                    if !status.starts_with("围栏刷新失败") {
                        self.boundary.fences = fences;
                    }
                    self.boundary.status = status;
                    self.osm_page.fitted = false;
                }
                LayerUpdate::Finished => self.checkpoints.busy = false,
            }
        }
    }
}
