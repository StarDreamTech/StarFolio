# 归档队列 / Archive queue

**录入不是归档。** 「加入归档队列」和 `folio enqueue <url>` 只校验链接格式、在本机保存待办；不打开网页、不解析正文、不向 Agent 上传内容。X 的分享链接按帖子 ID 去重；其他链接按去掉锚点后的完整地址去重，保留查询参数。重复提交不会覆盖已有状态。浏览器文件导入独立使用。旧的显式 CLI `folio save <url>` 仍可手动运行，但不是队列的处理方式。

## 本机 Agent 接口

队列位于与书架相同的数据目录：`tech.stardream.starfolio/library/archive-queue/`。桌面 CLI 可用 `STARFOLIO_LIBRARY` 指向隔离测试目录。不要把实际链接、浏览器导出、账号信息或归档提交到源码仓库；队列也不跨设备同步。单个 Agent 按添加顺序处理，不并发领取。

1. `folio queue` 读取本机 JSON 列表。只处理 `state == "pending"`，没任务时不要开浏览器；未配置调度时，队列不会自己运行。
2. 用正常浏览器在原页核对链接、正文、时间与配图，排除广告和推荐。核对是否已有同一来源，X 比对帖子 ID；同一条只补缺项。用原始浏览器导出与本地素材导入，**不要用 `folio save` 作为队列的降级方案**。英文主帖按[双语阅读规则](reading-language.md)补译并保留原文。
3. `folio verify <snapshot-id>` 并打开本地阅读页核对正文与图片。`folio queue-done <job-id> <snapshot-id>` 再核对来源身份、完整性后，原子写入完成状态。同来源 X 别名可匹配；其他域名或路径重定向不能自动视为同一篇，需核对再用原链接归档。
4. 需要登录、安全验证或暂时不能确认原文时，用 `folio queue-block <job-id> <简短原因>` 保留待办，不绕过。条件满足后，用户可点击「重新排队」或用 `folio queue-retry <job-id>` 重试。

**English:** Configure a separate Agent on the same machine. Read `folio queue`, process only `pending` jobs with evidence from a normal browser, and verify the offline copy. `folio queue-done <job-id> <snapshot-id>` requires a verified snapshot of the same source. Use `folio queue-block <job-id> <reason>` for blocked requests; retry by UI or `folio queue-retry <job-id>`. The link form never fetches or publishes content.
