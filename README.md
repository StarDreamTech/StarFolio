# 星藏 StarFolio

> 把值得重读的文章与讨论，留在自己的设备。 / Keep articles and discussions worth revisiting on your own device.

[English](README.en.md)

星藏是一款开源、本地优先的网页归档阅读器。提交链接或导入浏览器保存的页面，整理正文、可见评论、作者与个人资料链接、图片；用同站点共用的阅读样式重新打开。归档留在本机，不传到组织仓库。

## 能做什么

- Tauri 2 + Rust，桌面支持 macOS / Windows / Linux；项目配置还生成 iOS、Android 工程。
- 论坛讨论尽量读取公开帖子流，记录楼层、帖子 ID、用户 ID、主页、日期、回复楼层与当时能读取到的内容。
- 通用 HTML 正文及可识别评论提取；清除脚本、表单、隐藏内容、广告和常见推广区块。
- 图片按内容哈希去重、验证格式后保存在私人归档；原文仅保留为引用。
- 可编辑的站点级 CSS，只存本机并应用于该站所有归档。
- 浏览器可正常阅读时可导出 `starfolio.discourse-dom.v1` JSON，并在应用中导入。保存为文件须遵守网站使用规则；不绕过登录、验证码或付费墙。
- 命令行：`cargo run --manifest-path src-tauri/Cargo.toml --bin folio -- save <url>`、`list`、`verify <id>`。

## 开发

需要 Rust 1.90+、Node 22+。macOS 还需要 Xcode；Linux 需要 Tauri / WebKitGTK 系统开发依赖。iOS / Android 打包分别需要 Apple 签名工具链、Android SDK/NDK。

```sh
npm ci
npm test
npm run tauri -- dev
npm run tauri -- build
```

首次 Rust 构建需要下载 crates。Linux 依赖请参见 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)。

## 隐私与边界

- 公开源码只包含程序；下载内容写入当前用户的数据目录 `tech.stardream.starfolio/library/`，Unix 上目录权限为 0700；Windows 使用用户目录继承的权限。不要把该目录、浏览器导出文件或真实帖子内容提交到 GitHub。
- 归档不是加密保险库。磁盘、系统账号、云同步及备份的安全由使用者负责。
- 自动归档仅请求标准 HTTP(S) 公网地址、拒绝重定向到私网或从 HTTPS 降级；不跟随系统代理。单页最多 3000 楼，单个图片最多 12 MiB、每页 200 MiB。被站点验证拦截时，使用浏览器导出的公开可见内容或 HTML 导入，不自动破解验证。
- 网页可能改变、缺楼、热链失效或禁止复制；自动提取器无法识别每个站点结构。保存时会展示缺失项。
- 删除站点广告不改变原站，也不改变外链内容。

## 许可

MIT。归档内容属于其原作者及原站点；使用者应尊重版权、隐私、网站条款及适用法律，公开仓库不收集或再分发私人备份。

## AI 如何参与

默认先探测 **本机 Ollama**（固定 `127.0.0.1:11434`）及 `qwen2.5:3b`，不使用云模型或中转。陌生 HTML 站点由模型按 DOM 候选块编号判断正文、评论与噪声，不让模型重写原文；结构明确的 Discourse 优先使用真实楼层与作者字段。保存前再分批用模型检查正文/评论摘录，异常只标记，不删楼。正文 SHA-256、图片完整性、来源和缺失记录由代码检查。摘要式复核不是逐字事实核查。

要启用本地 AI，请先在 Ollama 中准备 `qwen2.5:3b`（`ollama pull qwen2.5:3b`），用 `ollama list` 确认，再保存链接。星藏不自动下载模型，也不接受云模型。

模型不可用、输出无效或超预算时仍保留可读快照，书架和「AI 复核」面板会说明状态，不伪称 AI 通过。本机准备好 Ollama 和模型后，可对已有快照选「AI 复核 → 重新复核」，或运行 `folio review <id>`，不会重新访问原站。此操作复核已保存的摘录，不会补回原本没抓到的内容。

对于可在浏览器正常阅读、自动下载却受限的论坛，打开源码中的 `tools/bookmark.html`，把导出按钮拖进书签栏；在主楼点击书签后正常向下阅读，再点计数按钮导出。星藏书架中选「导入浏览器保存的页面」。这只读取已经显示的楼层，不读取登录凭据、不自动翻页。普通文章可导入 UTF-8 HTML；不跟随本地文件路径，推荐含内嵌图片的单文件 HTML。

移动平台需准备相应工具链。Android 首次运行 `npm run tauri -- android init`，然后 `npm run tauri -- android build`；iOS 工程位于 `src-tauri/gen/apple`，用 `npm run tauri -- ios build`。构建检查与真机验证是两回事，请以具体平台 CI 结果为准。
