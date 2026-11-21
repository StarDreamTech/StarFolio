use crate::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
fn root(h: &AppHandle) -> Result<PathBuf> {
    Ok(h.path().app_local_data_dir()?.join("library"))
}
fn msg(e: anyhow::Error) -> String {
    format!("{e:#}")
}
#[tauri::command]
fn catalog(h: AppHandle) -> Result<(String, Vec<Summary>), String> {
    (|| {
        let r = root(&h)?;
        Ok((r.display().to_string(), list(&r)?))
    })()
    .map_err(msg)
}
#[tauri::command]
async fn save(h: AppHandle, url: String) -> Result<Summary, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || capture(&r, &url))
        .await
        .map_err(|e| e.to_string())?
        .map_err(msg)
}
#[tauri::command]
fn categorize(h: AppHandle, id: String, name: String) -> Result<(), String> {
    set_category(&root(&h).map_err(msg)?, &id, &name).map_err(msg)
}
#[tauri::command]
async fn import_file(h: AppHandle, url: String, content: String) -> Result<Summary, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || import(&r, &url, &content))
        .await
        .map_err(|e| e.to_string())?
        .map_err(msg)
}
#[tauri::command]
async fn review_again(h: AppHandle, id: String) -> Result<ai::Audit, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || recheck(&r, &id))
        .await
        .map_err(|e| e.to_string())?
        .map_err(msg)
}
#[derive(Serialize)]
struct View {
    html: String,
    css: String,
    site_css: String,
    audit: ai::Audit,
}
#[tauri::command]
async fn read(h: AppHandle, id: String) -> Result<View, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || {
        let c = snapshot(&r, &id)?;
        let css = fs::read_to_string(r.join("styles-private").join(site_css(&c.summary.site)))?;
        Ok(View {
            html: article(&c).replace(" src=\"images/", " data-local=\""),
            css: BASE_CSS.into(),
            site_css: css,
            audit: c.audit,
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(msg)
}
#[tauri::command]
fn style(h: AppHandle, id: String, css: String) -> Result<(), String> {
    (|| {
        ensure!(css.len() <= 64 * 1024, "样式超过 64 KiB");
        let r = root(&h)?;
        let path = folder(&r, &id)?.join("summary.json");
        let s: Summary = serde_json::from_slice(&fs::read(path)?)?;
        atomic(
            &r.join("styles-private").join(site_css(&s.site)),
            css.as_bytes(),
        )
    })()
    .map_err(msg)
}
#[tauri::command]
async fn image(h: AppHandle, id: String, name: String) -> Result<String, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (dir, ext) = name.split_once('.').context("无效的图片名")?;
        ensure!(
            key(dir) && matches!(ext, "png" | "jpg" | "webp" | "gif" | "avif"),
            "无效的图片名"
        );
        let path = folder(&r, &id)?.join("images").join(&name);
        ensure!(fs::metadata(&path)?.len() <= 12 << 20, "图片过大");
        let b = fs::read(path)?;
        ensure!(raster(&b) == Some(ext), "无效的格式");
        Ok(format!(
            "data:image/{};base64,{}",
            if ext == "jpg" { "jpeg" } else { ext },
            STANDARD.encode(b)
        ))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(msg)
}
#[tauri::command]
fn open_link(h: AppHandle, url: String) -> Result<(), String> {
    (|| {
        let u = Url::parse(&url)?;
        public(&u)?;
        h.opener().open_url(u.as_str(), None::<&str>)?;
        Ok(())
    })()
    .map_err(msg)
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            catalog,
            categorize,
            review_again,
            save,
            read,
            style,
            image,
            open_link,
            import_file
        ])
        .run(tauri::generate_context!())
        .expect("StarFolio startup failed")
}
