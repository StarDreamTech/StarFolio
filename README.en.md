# StarFolio 星藏

> Keep articles and discussions worth revisiting on your own device. / 把值得重读的文章与讨论，留在自己的设备。

[中文](README.md)

StarFolio is an open-source, local-first reader for archiving web pages. Paste a link or import a browser-saved page to preserve article text, visible comments, author/profile references, and images, then reread everything with a shared site-specific stylesheet. Captures stay in your private local data directory, not in the organization repository.

## Features

- Tauri 2 + Rust. Desktop targets: macOS, Windows, and Linux. Platform projects can also be generated for iOS and Android.
- For forum threads, capture publicly exposed posts where available: post number and ID, user ID, profile URL, timestamp, reply number, and visible body.
- Generic HTML article and detectable-comment extraction; removes scripts, forms, hidden content, ads, and common promotional regions.
- Deduplicates images by content hash, checks file formats, and saves them to the private archive. The original page remains a reference.
- Editable per-site CSS, stored locally and shared across captures from that site.
- For pages you can read normally in your browser, export a `starfolio.discourse-dom.v1` JSON file for import. Respect site rules; this app does not bypass sign-in, CAPTCHA, or paywalls.
- CLI: `cargo run --manifest-path src-tauri/Cargo.toml --bin folio -- save <url>`, `list`, and `verify <id>`.

## Development

Rust 1.90+ and Node 22+ are required. macOS builds also need Xcode; Linux needs Tauri/WebKitGTK system development packages. Packaging for iOS and Android requires the respective Apple-signing and Android SDK/NDK toolchains.

```sh
npm ci
npm test
npm run tauri -- dev
npm run tauri -- build
```

The first Rust build downloads crates. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for Linux packages.

## Privacy and limitations

- Public source contains the app, not captured content. Downloads go under the current user's app data directory at `tech.stardream.starfolio/library/`, with mode 0700 on Unix and inherited user-profile permissions on Windows. Never commit that directory, browser exports, or real posts.
- This is not an encrypted vault. Protect your OS account, disk, sync provider, and backups.
- Automatic fetches use standard HTTP(S) ports and public addresses only, reject private-network redirects and HTTPS downgrades, and ignore system proxies. Limits: 3,000 posts, 12 MiB per image, and 200 MiB total images per capture. If challenged, import publicly visible content from your browser; verification is not bypassed.
- Websites change; posts may be missing, hot-linked media may fail, and copying may be restricted. A generic extractor cannot understand every site's layout. Missing items are reported.
- Removing ads from a capture does not alter the original site or linked pages.

## License

MIT. Archived content remains subject to its authors and source sites. Respect copyright, privacy, site terms, and applicable law. This public repository neither collects nor redistributes private captures.

## How AI participates

StarFolio first checks **local Ollama** on the fixed loopback address `127.0.0.1:11434` for `qwen2.5:3b`. No hosted model or gateway is used. For unfamiliar HTML sites, the model classifies numbered source DOM blocks as article, comment, noise or unclear. It cannot rewrite text. Discourse uses authoritative post records first. A second pass reviews batches of excerpts; suspicious records are flagged rather than deleted. Code verifies per-body SHA-256, image integrity, provenance and missing IDs. This is a sampled quality check, not line-by-line fact checking.

To enable local AI, install the weights with `ollama pull qwen2.5:3b` and confirm with `ollama list` before saving a link. StarFolio neither downloads models automatically nor accepts cloud models.

Unavailable models, invalid answers and budget limits are recorded. The library and review panel distinguish pending review from completed review. Once a local Ollama model is ready, open the review panel or run `folio review <id>` to recheck a saved snapshot without refetching it. Rechecking cannot recover never-captured content.

If a discussion is normally readable in your browser but automated downloads are restricted, open the local `tools/bookmark.html` and drag its link to your bookmarks. Activate it on the first post and scroll normally; the counter exports only posts actually displayed. Import that JSON in the app. No cookie access or automated paging. Ordinary articles can be imported as UTF-8 HTML; embedded images are preferable because arbitrary local image paths are never opened.

On Android, run `npm run tauri -- android init`, then `npm run tauri -- android build`. For iOS, use the project at `src-tauri/gen/apple` via `npm run tauri -- ios build`. A successful build is not device testing; consult CI for each platform.

## Organizing and migrating archives (0.2)

Combine category, domain, AI tag and text filters; **Untagged** includes older snapshots. Use the reader's category editor to set a personal category, or clear it to restore the source category. The local model suggests up to five topic tags from the title and a main-text excerpt during capture. A missing model or invalid output never invents tags or clears previously assigned tags. Review again to retry without visiting the site.

Import legacy X v3 JSON from the shelf. A translated main post is rejected; translated replies explicitly marked in historical files are retained with a notice. `tools/x-bookmark.html` can export originals and replies you explicitly record while reading a public thread. No automatic paging, login handling, nested-reply expansion or video extraction; stop at a challenge. Status IDs, attribution and links, reply type, available timestamps, media and historical metrics are retained when the source contains them.

`folio import-collection <directory>` checks the catalog's record and local image hashes, never downloads missing images, and can resume without duplicating unchanged records. Set `STARFOLIO_LIBRARY` for an isolated CLI shelf. The GUI always uses its private data directory; there is no automatic GitHub publishing or sync.
