#![cfg(all(target_os = "macos", feature = "updater-smoke"))]

use std::{env, fs, path::PathBuf, time::Duration};
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri_plugin_updater::UpdaterExt;

// tests/updater-macos.py supplies the throwaway signed bundle and TLS server.
#[test]
#[ignore = "run with python3 tests/updater-macos.py"]
fn signed_update_over_https() {
    let fixture = PathBuf::from(env::var_os("STARFOLIO_UPDATER_TEST_FIXTURE").unwrap());
    let endpoint = env::var("STARFOLIO_UPDATER_TEST_ENDPOINT").unwrap();
    let pubkey = fs::read_to_string(fixture.join("updater.key.pub")).unwrap();
    let ca = reqwest::Certificate::from_pem(&fs::read(fixture.join("ca.crt")).unwrap()).unwrap();
    let installed = fixture.join("Installed.app/Contents/MacOS/fixture");
    let mut ctx = mock_context(noop_assets());
    ctx.package_info_mut().version = "0.2.2".parse().unwrap();
    ctx.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({"pubkey":pubkey.trim(),"requireSignedVersion":true}),
    );
    let app = mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(ctx)
        .unwrap();
    tauri::async_runtime::block_on(async {
        let updater = |route: &str| {
            let ca = ca.clone();
            app.updater_builder()
                .executable_path(&installed)
                .endpoints(vec![format!("{endpoint}/{route}").parse().unwrap()])
                .unwrap()
                .timeout(Duration::from_secs(4))
                .configure_client(move |client| {
                    client.https_only(true).tls_certs_only([ca.clone()])
                })
                .build()
                .unwrap()
        };
        for route in ["current", "older", "empty"] {
            assert!(updater(route).check().await.unwrap().is_none(), "{route}");
        }
        for route in ["tampered", "wrong-version"] {
            let update = updater(route).check().await.unwrap().unwrap();
            let error = update
                .download_and_install(|_, _| {}, || {})
                .await
                .unwrap_err();
            let detail = error.to_string().to_lowercase();
            assert!(
                detail.contains("signature") || detail.contains("version"),
                "{detail}"
            );
            assert_eq!(fs::read(&installed).unwrap(), b"old");
            println!("{route} rejected: {error}");
        }
        let valid = updater("valid").check().await.unwrap().unwrap();
        assert_eq!(valid.version, "0.2.3");
        valid.download_and_install(|_, _| {}, || {}).await.unwrap();
        assert_eq!(fs::read(&installed).unwrap(), b"new");
        println!("Signed macOS bundle downloaded over HTTPS and installed only into the fixture");
    });
}
