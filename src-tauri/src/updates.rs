use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
pub(super) struct AppInfo {
    version: String,
    update_supported: bool,
}

fn supported(h: &AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    if !std::env::current_exe().ok().is_some_and(|path| {
        path.parent()
            .and_then(|p| p.file_name())
            .is_some_and(|name| name == "MacOS")
            && path
                .ancestors()
                .nth(2)
                .and_then(|p| p.file_name())
                .is_some_and(|name| name == "Contents")
            && path
                .ancestors()
                .nth(3)
                .is_some_and(|p| p.extension().is_some_and(|ext| ext == "app"))
    }) {
        return false; // Do not replace a directory containing an unbundled binary.
    }
    cfg!(desktop)
        && h.config().plugins.0.get("updater").is_some_and(|config| {
            config["requireSignedVersion"] == true
                && config["pubkey"]
                    .as_str()
                    .is_some_and(|key| !key.trim().is_empty())
        })
}

#[tauri::command]
pub(super) fn app_info(h: AppHandle) -> AppInfo {
    AppInfo {
        version: h.package_info().version.to_string(),
        update_supported: supported(&h),
    }
}

#[cfg(desktop)]
async fn pending(h: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    use std::time::Duration;
    use tauri_plugin_updater::UpdaterExt;

    if !supported(h) {
        return Err("此安装包未启用签名更新。".into());
    }
    let update = h
        .updater_builder()
        .configure_client(|client| client.https_only(true))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(u) = &update {
        let url = &u.download_url;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || !url
                .path()
                .starts_with("/StarDreamTech/StarFolio/releases/download/")
        {
            return Err("更新包不在星藏官方 GitHub 发布路径。".into());
        }
    }
    Ok(update)
}

#[tauri::command]
pub(super) async fn check_update(h: AppHandle) -> Result<Option<String>, String> {
    #[cfg(desktop)]
    return Ok(pending(&h).await?.map(|update| update.version));
    #[cfg(not(desktop))]
    {
        let _ = h;
        Err("此平台不支持应用内更新。".into())
    }
}

#[tauri::command]
pub(super) async fn install_update(h: AppHandle, version: String) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use std::time::Duration;
        // Recheck after user consent; never install a different version from the one displayed.
        let mut update = pending(&h)
            .await?
            .filter(|u| u.version == version)
            .ok_or("更新版本已变化，请重新检查。")?;
        update.timeout = Some(Duration::from_secs(300));
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        #[cfg(not(windows))]
        h.request_restart();
        Ok(())
    }
    #[cfg(not(desktop))]
    {
        let _ = (h, version);
        Err("此平台不支持应用内更新。".into())
    }
}
