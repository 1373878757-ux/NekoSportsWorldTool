//! Read-only map loading. Retained display roads never authorize a stale route.
use super::{osm::NetworkLoad, App};
use crate::location::Coordinate;
use std::sync::Arc;

#[derive(Clone, PartialEq)]
pub(super) enum MapDisplayScope {
    LocalFile(String),
    School {
        account: Option<(i64, String)>,
        anchor: Option<Coordinate>,
        campus: Option<(String, String)>,
        offline_preview: bool,
    },
}

impl App {
    pub(super) fn map_display_scope(&self) -> MapDisplayScope {
        if !self.osm_page.offline && !self.config.osm_path.is_empty() {
            return MapDisplayScope::LocalFile(self.config.osm_path.clone());
        }
        MapDisplayScope::School {
            account: self.session.as_ref().map(|s| (s.uid, s.unid.clone())),
            anchor: self.identity.anchor_coordinate().ok(),
            campus: self
                .config
                .school_campus
                .as_ref()
                .map(|s| (s.school.clone(), s.key.clone())),
            offline_preview: self.osm_page.offline,
        }
    }

    pub(super) fn remember_display_map(&mut self, net: Arc<route_planner::RoadGraph>) {
        self.osm_page.display_scope = Some(self.map_display_scope());
        self.osm_page.display_network = Some(net);
    }

    pub(super) fn prepare_display_refresh(&mut self) {
        let scope = self.map_display_scope();
        if self
            .osm_page
            .display_scope
            .as_ref()
            .is_some_and(|old| old != &scope)
        {
            self.osm_page.display_network = None;
        } else if let Some(net) = self.network.clone() {
            self.remember_display_map(net);
        }
    }

    pub(super) fn displayed_road_network(&self) -> Option<Arc<route_planner::RoadGraph>> {
        if self
            .osm_page
            .display_scope
            .as_ref()
            .is_some_and(|scope| scope != &self.map_display_scope())
        {
            return None;
        }
        self.network
            .clone()
            .or_else(|| self.osm_page.display_network.clone())
    }

    pub(super) fn load_map_file_for_view(&mut self, path: String) {
        self.load_osm_async(path);
        self.osm_page.map_only_request = true;
    }

    /// One automatic attempt per source/account/location/campus, not per frame or tab entry.
    pub(super) fn ensure_map_loaded(&mut self) {
        #[cfg(test)]
        if !crate::runtime::offline() && self.route_fixture.is_none() {
            return; // Headless UI fixtures must opt in with an explicit loopback host.
        }
        if !self.interactive || self.login_busy || self.osm_page.busy || self.checkpoints.busy {
            return;
        }
        let scope = self.map_display_scope();
        if self.osm_page.auto_attempted && self.osm_page.auto_scope.as_ref() == Some(&scope) {
            return;
        }
        self.osm_page.auto_attempted = true;
        self.osm_page.auto_scope = Some(scope);
        if self.osm_page.offline {
            return;
        }
        if crate::runtime::offline() {
            self.refresh_checkpoint_layer();
            if self.network.is_none() && !self.config.osm_path.is_empty() {
                self.load_map_file_for_view(self.config.osm_path.clone());
            }
        } else if !self.config.osm_path.is_empty() {
            if self.network.is_none() {
                self.load_map_file_for_view(self.config.osm_path.clone());
            }
            if self.session.is_some() && !self.checkpoints.busy && !self.osm_page.busy {
                self.refresh_checkpoint_layer();
            }
        } else if self.session.is_some() {
            self.refresh_map_roads(false);
        } else {
            self.osm_page.status = "登录后会自动读取校园打卡点和道路；已有本地点位仍可查看".into();
        }
    }

    /// Map viewing shares the point/fence APIs without fetching a running policy.
    /// It does not change running mode or construct PreviewInputs.
    pub(super) fn refresh_map_roads(&mut self, force: bool) {
        if self.osm_page.busy || self.checkpoints.busy {
            return;
        }
        if crate::runtime::offline() {
            self.refresh_checkpoint_layer();
            self.osm_page.status = crate::runtime::OFFLINE_MESSAGE.into();
            return;
        }
        let Some(session) = self.session.clone() else {
            self.osm_page.status = "请先登录，才能识别学校与围栏".into();
            return;
        };
        let request = self.begin_checkpoint_load();
        let request_id = self.begin_network_load();
        self.osm_page.map_only_request = true;
        self.osm_page.status = "正在读取校园打卡点和道路；已加载内容仍可查看…".into();
        let identity = self.identity.clone();
        let anchor = identity.anchor_coordinate().ok();
        let point_anchor = anchor.filter(|_| !identity.has_unconfigured_default_location());
        let scope = Some((
            session.uid,
            session.unid.clone(),
            anchor,
            session.token.clone(),
        ));
        let selection = self.config.school_campus.clone();
        let layer_tx = self.layer_tx.clone();
        let net_tx = self.net_tx.clone();
        let control = self.osm_page.cancel.clone();
        #[cfg(test)]
        let fixture = self.route_fixture.clone();
        #[cfg(test)]
        let overpass = self.route_overpass_fixture.clone();
        self.spawn_session_job(move |tx| {
            let mut log = App::logger(tx.clone());
            let mut client = tx.client(identity, Some(session));
            #[cfg(test)]
            if let Some((host, directory)) = fixture {
                client.set_test_host(host);
                crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(directory));
            }
            #[cfg(test)]
            crate::api::school_roads::TEST_OVERPASS.with(|p| *p.borrow_mut() = overpass);
            let mut campuses = Vec::new();
            let mut selected_key = String::new();
            let mut warning = String::new();
            let result = (|| {
                control.check()?;
                let metadata = super::checkpoint_jobs::load_metadata(
                    &mut client,
                    point_anchor,
                    selection.as_ref(),
                    &request,
                    &layer_tx,
                    &mut log,
                )?;
                campuses = metadata.campuses;
                selected_key = metadata.selected_key;
                let points = match metadata.points {
                    Ok(points) => crate::api::points::points_bd(&points),
                    Err(error) if !metadata.fences.is_empty() => {
                        warning = format!("打卡点刷新未完成：{error}");
                        Vec::new()
                    }
                    Err(error) => return Err(error),
                };
                control.check()?;
                let school = client.login.as_ref().ok_or("未登录")?.unid.clone();
                let net = crate::api::school_roads::load_for_route_cancellable(
                    "",
                    &school,
                    &metadata.fences,
                    &points,
                    force,
                    &control,
                    &mut log,
                )?;
                control.check()?;
                Ok(Arc::new(net))
            })();
            let _ = layer_tx.send(super::checkpoint_jobs::LayerLoad {
                request,
                update: super::checkpoint_jobs::LayerUpdate::Finished,
            });
            let _ = net_tx.send(NetworkLoad {
                pending: false,
                warning,
                offline: false,
                request_id,
                campuses,
                selected_key,
                scope,
                inputs: None,
                result,
            });
        });
    }
}
