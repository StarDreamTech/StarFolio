//! Native rich + plain representations in a single clipboard item. No read access.
use crate::copy::Clip;
use anyhow::Result;
use tauri::WebviewWindow;

#[cfg(desktop)]
pub(crate) fn write(_: &WebviewWindow, clip: Clip) -> Result<()> {
    use std::sync::Mutex;
    // On Linux the clipboard owner must live beyond the copy command.
    static OWNER: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);
    let mut owner = OWNER
        .lock()
        .map_err(|_| anyhow::anyhow!("剪贴板忙，请重试"))?;
    if owner.is_none() {
        *owner = Some(arboard::Clipboard::new()?);
    }
    owner
        .as_mut()
        .unwrap()
        .set_html(clip.html, Some(clip.text))?;
    Ok(())
}

#[cfg(target_os = "ios")]
pub(crate) fn write(w: &WebviewWindow, clip: Clip) -> Result<()> {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_ui_kit::UIPasteboard;
    let (tx, rx) = std::sync::mpsc::channel();
    w.run_on_main_thread(move || {
        let keys = [
            NSString::from_str("public.html"),
            NSString::from_str("public.utf8-plain-text"),
        ];
        let data = NSData::with_bytes(clip.html.as_bytes());
        let text = NSData::with_bytes(clip.text.as_bytes());
        let item = NSDictionary::<NSString, AnyObject>::from_slices(
            &[&*keys[0], &*keys[1]],
            &[data.as_ref(), text.as_ref()],
        );
        // Each value is NSData keyed by its representation's UTI, as required by UIKit.
        unsafe {
            UIPasteboard::generalPasteboard().setItems(&NSArray::from_slice(&[&*item]));
        }
        let _ = tx.send(());
    })?;
    rx.recv()?;
    Ok(())
}

#[cfg(target_os = "android")]
pub(crate) fn write(w: &WebviewWindow, clip: Clip) -> Result<()> {
    // ClipboardManager uses Binder with a shared transaction buffer. Refuse large clips
    // before replacing the primary clip, rather than silently losing its pictures.
    anyhow::ensure!(
        clip.html.encode_utf16().count() + clip.text.encode_utf16().count() <= 384 * 1024,
        "图文超过 Android 系统剪贴板的安全容量，未复制；如包含评论，可取消勾选后重试"
    );
    let (tx, rx) = std::sync::mpsc::channel();
    w.with_webview(move |v| v.jni_handle().exec(move |env, activity, _| {
        let r=(|| -> jni::errors::Result<()> {
            let service=env.new_string("clipboard")?;
            let manager=env.call_method(activity,"getSystemService","(Ljava/lang/String;)Ljava/lang/Object;", &[(&service).into()])?.l()?;
            let label=env.new_string("StarFolio")?;
            let text=env.new_string(clip.text)?;
            let html=env.new_string(clip.html)?;
            let data=env.call_static_method("android/content/ClipData","newHtmlText","(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/String;)Landroid/content/ClipData;", &[(&label).into(),(&text).into(),(&html).into()])?.l()?;
            env.call_method(manager,"setPrimaryClip","(Landroid/content/ClipData;)V", &[(&data).into()])?;
            Ok(())
        })();
        if r.is_err() { let _=env.exception_clear(); }
        let _=tx.send(r.map_err(|err|err.to_string()));
    }))?;
    rx.recv()?.map_err(anyhow::Error::msg)
}
