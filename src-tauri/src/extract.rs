use crate::{network::link, Entry};
use anyhow::{ensure, Result};
use ego_tree::NodeRef;
use scraper::{node::Node, ElementRef, Html, Selector};
use url::Url;
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}
fn is_comment(el: &ElementRef) -> bool {
    let e = el.value();
    e.attr("itemtype").is_some_and(|x| x.ends_with("/Comment"))
        || e.attr("data-comment-id").is_some()
        || e.classes().any(|v| v == "comment")
}
fn noise(e: &scraper::node::Element) -> bool {
    if e.attr("hidden").is_some()
        || e.attr("aria-hidden") == Some("true")
        || e.attr("data-ad-slot").is_some()
    {
        return true;
    }
    let toks = e.classes().chain(e.attr("id"));
    toks.into_iter().any(|t| {
        matches!(
            t,
            "ad" | "ads"
                | "advert"
                | "ad-banner"
                | "advertisement"
                | "adsbygoogle"
                | "sponsored"
                | "social-share"
                | "newsletter"
                | "cookie-banner"
                | "cookie-consent"
                | "related-posts"
                | "sidebar"
                | "lightbox-meta"
        ) || t.starts_with("advertisement-")
            || t.starts_with("ad-slot")
    })
}
pub type ImageFn<'a> = dyn FnMut(&Url) -> Option<String> + 'a;
fn children(
    el: ElementRef,
    base: &Url,
    image: &mut ImageFn,
    skip_comments: bool,
    classified: bool,
    depth: u16,
) -> String {
    el.children()
        .map(|n| render(n, base, image, skip_comments, classified, depth + 1))
        .collect()
}
fn render(
    n: NodeRef<'_, Node>,
    base: &Url,
    image: &mut ImageFn,
    skip_comments: bool,
    classified: bool,
    depth: u16,
) -> String {
    if depth > 160 {
        return String::new();
    }
    if let Node::Text(t) = n.value() {
        return esc(t);
    }
    let Some(el) = ElementRef::wrap(n) else {
        return String::new();
    };
    let e = el.value();
    let tag = e.name();
    if e.attr("hidden").is_some()
        || e.attr("aria-hidden") == Some("true")
        || (!classified && (noise(e) || matches!(tag, "nav" | "footer")))
        || skip_comments && is_comment(&el)
        || matches!(
            tag,
            "script"
                | "style"
                | "form"
                | "iframe"
                | "svg"
                | "math"
                | "object"
                | "video"
                | "audio"
                | "embed"
                | "noscript"
                | "template"
                | "button"
                | "input"
                | "head"
                | "canvas"
        )
    {
        return String::new();
    }
    if !classified && tag == "aside" && !e.classes().any(|c| c == "quote" || c == "onebox") {
        return String::new();
    }
    if tag == "br" || tag == "hr" {
        return format!("<{tag}>");
    }
    if tag == "img" {
        if (e.classes().any(|c| c == "avatar") || e.attr("itemprop") == Some("image"))
            && el
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|v| v.value().name() == "a")
                .is_some_and(|a| {
                    a.value().attr("rel") == Some("author")
                        || a.value().attr("data-user-card").is_some()
                        || a.value().classes().any(|c| c == "author")
                        || a.ancestors()
                            .filter_map(ElementRef::wrap)
                            .any(|v| v.value().classes().any(|c| c == "comment-author"))
                })
        {
            return String::new();
        }
        let alt = esc(e.attr("alt").unwrap_or("图片"));
        if e.attr("width") == Some("1") || e.attr("height") == Some("1") {
            return String::new();
        }
        let src = e
            .attr("data-original")
            .or(e.attr("data-src"))
            .or(e.attr("data-lazy-src"))
            .or_else(|| {
                e.attr("srcset")
                    .and_then(|v| v.split(',').next_back()?.split_whitespace().next())
            })
            .or(e.attr("src"));
        let Some(url) = src.and_then(|s| {
            if s.starts_with("data:image/") {
                Url::parse(s).ok()
            } else {
                link(base, s)
            }
        }) else {
            return alt;
        };
        let full = el
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|a| a.value().name() == "a" && a.value().classes().any(|c| c == "lightbox"))
            .and_then(|a| link(base, a.value().attr("href")?));
        let local = full
            .as_ref()
            .filter(|v| *v != &url)
            .and_then(&mut *image)
            .or_else(|| image(&url));
        return match local {
            Some(path) => format!(
                "<img loading=\"lazy\" class=\"{}\" src=\"{}\" alt=\"{alt}\">",
                if e.classes().any(|c| c == "emoji" || c == "avatar") {
                    "inline-image"
                } else {
                    "illustration"
                },
                esc(&path)
            ),
            None => format!("<span class=\"missing\">[图片未保存：{alt}]</span>"),
        };
    }
    let body = children(el, base, image, skip_comments, classified, depth);
    if tag == "a" {
        if let Some(u) = e.attr("href").and_then(|s| link(base, s)) {
            return format!(
                "<a href=\"{}\" rel=\"noreferrer noopener\">{body}</a>",
                esc(u.as_str())
            );
        }
        return body;
    }
    let tag = match tag {
        "aside" => "blockquote",
        "section" | "article" | "main" => "div",
        "p" | "div" | "span" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "pre"
        | "code" | "em" | "strong" | "b" | "i" | "u" | "ul" | "ol" | "li" | "table" | "thead"
        | "tbody" | "tr" | "td" | "th" | "del" | "figure" | "figcaption" | "details"
        | "summary" | "sup" | "sub" | "kbd" => tag,
        _ => return body,
    };
    format!("<{tag}>{body}</{tag}>")
}
pub fn fragment(html: &str, base: &Url, image: &mut ImageFn) -> String {
    let doc = Html::parse_fragment(html);
    children(doc.root_element(), base, image, false, false, 0)
}
/// Model-selected blocks still pass the same URL, attribute and active-content sanitizer.
pub fn classified_fragment(html: &str, base: &Url, image: &mut ImageFn) -> String {
    let doc = Html::parse_fragment(html);
    children(doc.root_element(), base, image, false, true, 0)
}
fn txt(e: ElementRef) -> String {
    e.text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn select_text(el: ElementRef, query: &str) -> Option<String> {
    el.select(&sel(query))
        .next()
        .map(txt)
        .filter(|v| !v.is_empty())
}
/// Only accept an explicit author link owned by this block, never one in a quote or a nested comment.
pub(crate) fn author_link<'a>(scope: ElementRef<'a>, query: &str) -> Option<ElementRef<'a>> {
    scope.select(&sel(query)).find(|a| {
        a.ancestors()
            .filter_map(ElementRef::wrap)
            .take_while(|v| v.id() != scope.id())
            .all(|v| {
                !is_comment(&v)
                    && v.value().name() != "blockquote"
                    && !v.value().classes().any(|c| c == "quote")
            })
    })
}
pub(crate) fn author_photo(a: Option<ElementRef>, base: &Url) -> Option<String> {
    let el = a?.select(&sel("img.avatar, img[itemprop=image]")).next()?;
    link(
        base,
        el.value()
            .attr("data-src")
            .or_else(|| el.value().attr("src"))?,
    )
    .map(Into::into)
}
pub fn generic(html: &str, base: &Url, image: &mut ImageFn) -> Result<(String, Vec<Entry>)> {
    let doc = Html::parse_document(html);
    let root = doc.root_element();
    let main = [
        "article",
        "[itemprop=articleBody]",
        "main",
        "[role=main]",
        "body",
    ]
    .iter()
    .find_map(|s| {
        doc.select(&sel(s))
            .max_by_key(|e| e.text().map(str::len).sum::<usize>())
    })
    .unwrap_or(root);
    let title = doc
        .select(&sel("meta[property='og:title']"))
        .next()
        .and_then(|e| e.value().attr("content"))
        .map(String::from)
        .or_else(|| select_text(main, "h1"))
        .or_else(|| select_text(root, "title"))
        .unwrap_or_else(|| base.host_str().unwrap_or("文章").into());
    let byline = doc
        .select(&sel("meta[name=author]"))
        .next()
        .and_then(|e| e.value().attr("content"))
        .map(String::from)
        .or_else(|| select_text(main, "[rel=author], [itemprop=author], .byline"));
    let a = author_link(main, "a[rel=author]");
    let mut out = vec![Entry {
        number: 1,
        url: base.to_string(),
        user: byline,
        avatar_url: author_photo(a, base),
        profile: a
            .and_then(|e| link(base, e.value().attr("href")?))
            .map(Into::into),
        html: children(main, base, image, true, false, 0),
        ..Default::default()
    }];
    ensure!(
        Html::parse_fragment(&out[0].html)
            .root_element()
            .text()
            .any(|t| !t.trim().is_empty())
            || out[0].html.contains("<img "),
        "未读到正文；可能需要登录或 JavaScript"
    );
    let cs = sel(
        "[itemtype$='/Comment'], [data-comment-id], #comments .comment, .comment-list .comment",
    );
    for (i, c) in doc.select(&cs).enumerate() {
        let a = author_link(
            c,
            "a[rel=author], .comment-author a, a.author, a[itemprop=author]",
        );
        let user = a
            .map(txt)
            .or_else(|| select_text(c, "[itemprop=author],.comment-author,.author"));
        let mut u = base.clone();
        if let Some(id) = c.value().attr("id") {
            u.set_fragment(Some(id))
        }
        let b = c
            .select(&sel(".comment-content, [itemprop=text]"))
            .next()
            .unwrap_or(c);
        out.push(Entry {
            number: (i + 2) as i64,
            url: u.into(),
            user,
            uid: c.value().attr("data-user-id").map(String::from),
            avatar_url: author_photo(a, base),
            profile: a
                .and_then(|e| link(base, e.value().attr("href")?))
                .map(Into::into),
            date: c
                .select(&sel("time[datetime]"))
                .next()
                .and_then(|e| e.value().attr("datetime"))
                .map(String::from),
            html: children(b, base, image, true, false, 0),
            ..Default::default()
        });
    }
    Ok((title, out))
}
