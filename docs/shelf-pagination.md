# 书架分页验收 / Shelf pagination verification

- `npm test`：合成书架覆盖首末页、翻页、空结果、筛选重置、作者总数和阅读页返回。/ Synthetic shelf checks cover page boundaries, navigation, no matches, filter resets, author counts, and returning from the reader.
- Apple 芯片 macOS 试用包：实测上一页、下一页、打开归档并返回原页；通过本地签名与 ZIP 校验。/ The Apple silicon preview was tested for navigating both ways and returning from the reader to the same page. Ad hoc signature and ZIP integrity checks passed.
- 未在其他桌面平台或移动真机手动验收。/ No manual verification on other desktop platforms or mobile devices.
