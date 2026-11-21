use crate::*;
#[path = "updates.rs"]
mod updates;
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
async fn catalog(h: AppHandle) -> Result<(String, Vec<Summary>), String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || Ok((r.display().to_string(), list(&r)?)))
        .await
        .map_err(|e| e.to_string())?
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
    source_url: String,
    audit: ai::Audit,
}
#[tauri::command]
async fn read(h: AppHandle, id: String) -> Result<View, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || {
        let c = snapshot(&r, &id)?;
        let css = fs::read_to_string(r.join("styles-private").join(site_css(&c.summary.site)))?;
        Ok(View {
            html: article(&c, true)
                .replace(" src=\"images/", " data-local=\"")
                .replace(" src=\"../../avatars/objects/", " data-avatar=\""),
            css: BASE_CSS.into(),
            site_css: css,
            source_url: c.summary.url,
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
        let (mime, b) = copy::image_bytes(&folder(&r, &id)?, &name)?;
        Ok(format!("data:image/{mime};base64,{}", STANDARD.encode(b)))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(msg)
}
#[tauri::command]
async fn avatar_image(h: AppHandle, name: String) -> Result<String, String> {
    let r = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (m, b) = avatars::bytes(&r.join("avatars"), &name)?;
        Ok(format!("data:image/{m};base64,{}", STANDARD.encode(b)))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(msg)
}
#[tauri::command]
async fn copy_article(
    h: AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    replies: bool,
) -> Result<(usize, usize), String> {
    if window.label() != "main" {
        return Err("只能从阅读页复制".into());
    }
    let root = root(&h).map_err(msg)?;
    tauri::async_runtime::spawn_blocking(move || {
        let clip = copy::prepare(&root, &id, replies)?;
        let counts = (clip.images, clip.missing);
        clipboard::write(&window, clip)?;
        Ok(counts)
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
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder.invoke_handler(tauri::generate_handler![
            updates::app_info,
            updates::check_update,
            updates::install_update,
            catalog,
            categorize,
            copy_article,
            review_again,
            save,
            read,
            style,
            image,
            avatar_image,
            open_link,
            import_file
        ])
        .run(tauri::generate_context!())
        .expect("StarFolio startup failed")
}
