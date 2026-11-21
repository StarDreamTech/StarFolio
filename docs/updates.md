# 星藏桌面更新

源码中的更新地址固定为本项目 GitHub Releases 的 `latest.json`，只通过 HTTPS 检查和下载；清单中的下载地址必须在本仓库的正式发布资产路径下。发行包需内置维护者的更新**公钥**。默认构建没配置公钥，不会发起更新请求；Android/iOS 不启用更新器。桌面更新与书架目录分开。

## 准备签名构建（不发布）

维护者建立并保护一对生产更新密钥。用 `npm run tauri -- signer generate --help` 查看生成命令；私钥和密码只用于获授权的打包环境，不能进仓库、安装包、日志或 `latest.json`。不能用测试密钥生成正式包。换生产密钥前需安排旧客户端的过渡。

在仓库外创建 Tauri 构建覆盖文件，只填生产**公钥**：

```json
{
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": { "updater": { "pubkey": "BASE64_PUBLIC_KEY_FROM_TAURI_SIGNER" } }
}
```

用受保护的 `TAURI_SIGNING_PRIVATE_KEY`（及需要时的 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）构建。示例：macOS 用 `npm run tauri -- build --ci --bundles app --config "$UPDATE_CONFIG"`，Windows 使用相应的 NSIS/MSI 包，Linux 用所支持的 AppImage 或发行包。Tauri CLI 在签名中绑定版本；手动签包时显式传 `--app-version`。不要移除现有的 `requireSignedVersion`，也不要把试用包的临时系统签名当成正式 macOS 签名或公证。

准备正式发布时，各目标平台的资产和 `.sig` 要先备齐；`latest.json` 用同一应用版本和各平台的 `url`、`signature`，其中 `signature` 是对应 `.sig` 的内容，`url` 是本仓库 `https://github.com/StarDreamTech/StarFolio/releases/download/<tag>/<asset>`。macOS Apple 芯片的目标名例如 `darwin-aarch64`。清单与资产必须放到同一个正式发布下。不要用预览包或仅部分平台的清单覆盖已有更新渠道。

## 本地验证（不碰已安装应用）

运行 `npm test` 验证界面，包括默认不外联、打开 GitHub、启动自动检查、发现新版本后等待确认，以及失败后重试。macOS 上运行 `python3 tests/updater-macos.py`，需要先 `npm ci`，以及 Rust、Python 和 OpenSSL。测试在临时目录签名，通过本机 HTTPS 下载并替换合成的 `.app`；测试证书仅对测试进程受信，不改系统信任。测试覆盖已是最新、不降级、被篡改的包、清单冒报版本和签名正确的安装。不访问 GitHub、不启动合成应用、不发布版本。
