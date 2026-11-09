pub fn explain(error: &str) -> String {
    let (kind, help) = if crate::api::client::is_session_expired(error) {
        (
            "登录已失效",
            "在账号栏重新登录；成功后会重新获取学校围栏和路网，本地地图仍可查看",
        )
    } else if error.contains("取消") {
        (
            "已取消",
            "可在当前操作结束后重试；已发出的网络请求不能立刻撤回",
        )
    } else if error.contains("[road:http]") && error.contains("HTTP 502") {
        (
            "Overpass HTTP 502",
            "服务端或中间网关未能处理请求；稍后再试，已有缓存和平台打卡点仍可查看",
        )
    } else if error.contains("timeout") || error.contains("timed out") || error.contains("超时") {
        ("网络超时", "检查网络后重试，或从缓存离线预览")
    } else if error.contains("[road:parse]") {
        (
            "OSM 响应解析失败",
            "检查响应是否完整；该错误不能用于判断学校道路是否为空",
        )
    } else if error.contains("[road:graph]") {
        (
            "OSM 构图无可用道路",
            "核对下载范围、highway 筛选和缺失节点计数",
        )
    } else if error.contains("[road:clip]") {
        (
            "围栏裁剪后无道路",
            "核对 OSM 覆盖、校区边界与 BD-09 坐标对齐；不能仅据此认定学校没有道路",
        )
    } else if error.contains("没有可用") || error.contains("没有可跑") || error.contains("空路网")
    {
        (
            "学校缺少道路数据",
            "更新路网或手动导入完整 OSM；不会向校外补足距离",
        )
    } else if error.contains("围栏") {
        (
            "围栏或校区问题",
            "在线刷新校区列表、核对设备定位，并确认学校边界",
        )
    } else if error.contains("不可达") || error.contains("不连通") || error.contains("超过 40")
    {
        ("点位不可达", "查看地图红色问题点，确认校区并补充缺失道路")
    } else if error.contains("不足") {
        ("目标距离不足", "按路线检查给出的最短距离增大目标里程")
    } else if error.contains("闭环") || error.contains("有效路线") {
        (
            "校内道路无法闭合",
            "更新或补充校内道路，检查单向限制和断开的道路",
        )
    } else if error.contains("下载")
        || error.contains("network")
        || error.contains("http")
        || error.contains("HTTP")
    {
        ("路网网络请求失败", "检查网络后重试；已有缓存可离线查看")
    } else {
        ("处理失败", "查看具体原因，修正输入后重试")
    };
    format!("[{kind}] {error}\n建议：{help}")
}
/// Concise map copy; detailed HTTP/parser diagnostics stay in logs/advanced controls.
pub fn map_summary(status: &str) -> String {
    if status.contains("HTTP 502") {
        "公共地图服务暂时不可用（HTTP 502）；打卡点仍可查看，可稍后重新加载。".into()
    } else if status.contains("[road:rate-limit]") {
        "公共地图请求间隔不足，请稍后重新加载；已有点位和地图仍可查看。".into()
    } else if status.contains("超时") || status.contains("timeout") {
        "地图服务响应超时；已有点位和地图仍可查看，可稍后重新加载。".into()
    } else if status.contains("[road:clip]") {
        "下载的道路未落入当前校园边界，请核对校区；打卡点仍可查看。".into()
    } else if status.contains("[road:parse]") {
        "地图文件或服务返回的内容无效；请核对来源，详情见高级选项和运行日志。".into()
    } else if status.contains("[road:graph]") {
        "地图数据中没有可显示的道路；请核对校区和数据范围，打卡点仍可查看。".into()
    } else if status.contains("地图来源设置保存失败") {
        "地图已加载，但来源设置保存失败；重启后可能恢复旧选择，详情见高级选项。".into()
    } else if status.contains("路网加载成功") {
        "校园道路已加载；点位、围栏和路线状态分别显示。".into()
    } else {
        status.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn failures_have_distinct_actions() {
        for (e, label) in [
            (crate::api::client::SESSION_EXPIRED, "登录已失效"),
            ("timeout", "网络超时"),
            ("没有可用的 OSM 道路", "学校缺少道路数据"),
            ("没有有效围栏", "围栏或校区问题"),
            ("点位不可达", "点位不可达"),
            ("目标距离不足", "目标距离不足"),
            ("[road:http] HTTP 502", "Overpass HTTP 502"),
            ("[road:http] HTTP 502 响应摘要=timeout", "Overpass HTTP 502"),
            ("[road:parse] XML 错误", "OSM 响应解析失败"),
            ("[road:graph] 没有可用道路", "OSM 构图无可用道路"),
            ("[road:clip] 学校围栏内没有可用道路", "围栏裁剪后无道路"),
        ] {
            assert!(super::explain(e).starts_with(&format!("[{label}]")));
        }
    }
}
