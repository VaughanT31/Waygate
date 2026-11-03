//! Public data only, no keys and no account (scope.md section 4):
//! - the latest version of each game from Blizzard's public patch server,
//!   the same `versions` table the game folder's `.build.info` comes from;
//! - news: articles from each game's official news page (title, summary,
//!   date and featured image), plus the first post of each topic in the
//!   Blizzard tracker on the official forums (posts by Blizzard staff:
//!   hotfixes, tuning), served as public JSON.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub fn agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new();
    if let Ok(tls) = native_tls::TlsConnector::new() {
        builder = builder.tls_connector(std::sync::Arc::new(tls));
    }
    builder
        .timeout(Duration::from_secs(12))
        .user_agent(concat!("Waygate/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// The patch server's region for a .build.info branch (eu, us, kr, tw, cn).
fn patch_region(region: &str) -> &'static str {
    match region.to_lowercase().as_str() {
        "eu" => "eu",
        "kr" => "kr",
        "tw" => "tw",
        "cn" => "cn",
        _ => "us",
    }
}

/// The latest version of a product for a region: ("12.1.5.70575", build fingerprint).
pub fn latest_version(product: &str, region: &str) -> Result<(String, String), String> {
    if !product.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Unknown product.".into());
    }
    let host = patch_region(region);
    let url = format!("http://{host}.patch.battle.net:1119/{product}/versions");
    let text = agent().get(&url).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    let mut lines = text.lines().filter(|l| !l.starts_with("##") && !l.trim().is_empty());
    let header: Vec<&str> = lines.next().ok_or("Empty answer")?.split('|').map(|c| c.split('!').next().unwrap_or("")).collect();
    let region_col = header.iter().position(|c| *c == "Region").ok_or("No Region column")?;
    let version_col = header.iter().position(|c| *c == "VersionsName").ok_or("No VersionsName column")?;
    let build_col = header.iter().position(|c| *c == "BuildConfig");
    let wanted = region.to_lowercase();
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split('|').collect()).collect();
    let row = rows.iter().find(|r| r.get(region_col) == Some(&wanted.as_str()))
        .or_else(|| rows.iter().find(|r| r.get(region_col) == Some(&host)))
        .or_else(|| rows.first())
        .ok_or("No versions listed")?;
    let version = row.get(version_col).map(|v| v.to_string()).ok_or("No version")?;
    let build = build_col.and_then(|i| row.get(i)).map(|b| b.to_lowercase()).unwrap_or_default();
    Ok((version, build))
}

#[derive(Serialize, Clone)]
pub struct NewsItem {
    pub uid: String,
    pub title: String,
    pub url: String,
    pub date: String,
    pub excerpt: String,
    /// The article's featured image, when it has one (forum posts don't).
    pub image: Option<String>,
}

/// Each game's forum on the official Blizzard forums.
fn forum_of(uid: &str) -> Option<&'static str> {
    match uid {
        "wow" => Some("wow"),
        "prometheus" => Some("overwatch"),
        _ => None,
    }
}

/// The forum host for a region: Europe has its own, everyone else reads the US one.
fn forum_host(region: &str) -> &'static str {
    if region.eq_ignore_ascii_case("eu") { "eu.forums.blizzard.com" } else { "us.forums.blizzard.com" }
}

/// Only Blizzard's own news and forum pages can be opened from news.
pub fn is_news_url(url: &str) -> bool {
    [
        "https://eu.forums.blizzard.com/en/",
        "https://us.forums.blizzard.com/en/",
        "https://worldofwarcraft.blizzard.com/",
        "https://worldofwarcraft.com/",
        "https://overwatch.blizzard.com/",
    ]
    .iter()
    .any(|p| url.starts_with(p))
        && !url.contains(['"', ' ', '<', '>'])
}

/// Only images on Blizzard's own image servers are shown (the window allows
/// just these hosts too, see tauri.conf.json).
fn image_url(url: &str) -> Option<String> {
    let url = if url.starts_with("//") { format!("https:{url}") } else { url.to_string() };
    let ok = ["https://bnetcmsus-a.akamaihd.net/", "https://blz-contentstack-images.akamaized.net/"].iter().any(|p| url.starts_with(p))
        && !url.contains(['"', ' ', '<', '>', '\'']);
    ok.then_some(url)
}

/// The news pages' language for a region.
fn locale(region: &str) -> &'static str {
    if region.eq_ignore_ascii_case("eu") { "en-gb" } else { "en-us" }
}

/// 2026-10-20T19:15:17Z from milliseconds since 1970 (days to a calendar date).
pub fn iso_from_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Now, in the same ISO format as the dates Raider.IO and the news use.
pub fn now_iso() -> String {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    iso_from_ms(ms)
}

/// WoW's news page carries its article list as JSON (`<script id="model">model = {...}`).
fn wow_articles(region: &str) -> Result<Vec<NewsItem>, String> {
    let loc = locale(region);
    let page = agent().get(&format!("https://worldofwarcraft.blizzard.com/{loc}/news")).call()
        .map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    let start = page.find("<script id=\"model\">").ok_or("No article data")?;
    let body = &page[start..];
    let json_start = body.find('{').ok_or("No article data")?;
    let json_end = body.find("</script>").ok_or("No article data")?;
    let raw = body[json_start..json_end].trim().trim_end_matches(';');
    let model: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let blogs = model.pointer("/blogList/blogs").and_then(|b| b.as_array()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for blog in blogs {
        let text = |k: &str| blog.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        // The list has had two shapes: shortUrl/url, published, description
        // and the older default_url, publish (ms), summary. Both are read.
        let first = |keys: &[&str]| keys.iter().map(|k| text(k)).find(|v| !v.is_empty()).unwrap_or_default();
        let title = text("title");
        let mut url = first(&["shortUrl", "default_url"]);
        if url.is_empty() {
            let path = text("url");
            if path.starts_with('/') {
                url = format!("https://worldofwarcraft.blizzard.com/{loc}{path}");
            }
        }
        if title.is_empty() || !is_news_url(&url) {
            continue;
        }
        let image = ["/thumbnail/url", "/image/url", "/header/url"].iter()
            .find_map(|p| blog.pointer(p).and_then(|u| u.as_str()).and_then(image_url));
        let date = match first(&["published", "created_at"]) {
            d if !d.is_empty() => d,
            _ => blog.get("publish").and_then(|p| p.as_i64()).map(iso_from_ms).unwrap_or_default(),
        };
        let excerpt = strip_html(&first(&["description", "summary"]));
        out.push(NewsItem { uid: "wow".into(), title: strip_html(&title), url, date, excerpt, image });
    }
    Ok(out)
}

/// The text between `start` and `end` after position `from`, and where it ended.
fn between<'a>(s: &'a str, from: usize, start: &str, end: &str) -> Option<(&'a str, usize)> {
    let a = s[from..].find(start)? + from + start.len();
    let b = s[a..].find(end)? + a;
    Some((&s[a..b], b))
}

/// Overwatch's news page lists its latest articles as cards: link, image, title, date.
fn overwatch_articles(region: &str) -> Result<Vec<NewsItem>, String> {
    let loc = locale(region);
    let page = agent().get(&format!("https://overwatch.blizzard.com/{loc}/news/")).call()
        .map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut at = 0;
    while let Some((href, after)) = between(&page, at, "<a slot=\"gallery-items\" href=\"", "\"") {
        let card_end = page[after..].find("</a>").map(|e| e + after).unwrap_or(page.len());
        let card = &page[after..card_end];
        let title = between(card, 0, "<h3 slot=\"heading\">", "</h3>").map(|(t, _)| strip_html(t)).unwrap_or_default();
        let image = between(card, 0, "src=\"", "\"").and_then(|(u, _)| image_url(u));
        let date = between(card, 0, "timestamp=\"", "\"").map(|(d, _)| d.to_string()).unwrap_or_default();
        let url = if href.starts_with('/') { format!("https://overwatch.blizzard.com/{loc}{href}") } else { href.to_string() };
        if !title.is_empty() && is_news_url(&url) {
            out.push(NewsItem { uid: "prometheus".into(), title, url, date, excerpt: String::new(), image });
        }
        at = card_end;
    }
    Ok(out)
}

pub fn articles(uid: &str, region: &str) -> Result<Vec<NewsItem>, String> {
    match uid {
        "wow" => wow_articles(region),
        "prometheus" => overwatch_articles(region),
        _ => Ok(Vec::new()),
    }
}

/// Letters and digits only, for spotting a forum post that repeats an article.
fn title_key(title: &str) -> String {
    title.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Text from an excerpt's HTML: tags dropped, common entities decoded.
fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&#x27;", "'").replace("&#039;", "'").replace("&rsquo;", "\u{2019}")
        .replace("&hellip;", "\u{2026}").replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

static NEWS_CACHE: Mutex<Option<(Instant, String, Vec<NewsItem>)>> = Mutex::new(None);
const NEWS_FRESH: Duration = Duration::from_secs(15 * 60);

pub fn forum_news(uid: &str, region: &str) -> Result<Vec<NewsItem>, String> {
    let forum = forum_of(uid).ok_or("No forum")?;
    let host = forum_host(region);
    let url = format!("https://{host}/en/{forum}/groups/blizzard-tracker/posts.json");
    let json: serde_json::Value = agent().get(&url).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let posts = json.get("posts").and_then(|p| p.as_array()).cloned().unwrap_or_default();
    let mut out: Vec<NewsItem> = Vec::new();
    for post in posts {
        // Only the post that starts a topic: announcements, not replies.
        if post.get("post_number").and_then(|n| n.as_u64()) != Some(1) {
            continue;
        }
        let title = post.get("topic_title").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
        let path = post.get("url").and_then(|u| u.as_str()).unwrap_or("");
        if title.is_empty() || !path.starts_with('/') || out.iter().any(|n| n.title == title) {
            continue;
        }
        out.push(NewsItem {
            uid: uid.to_string(),
            url: format!("https://{host}/en/{forum}{path}"),
            date: post.get("created_at").and_then(|d| d.as_str()).unwrap_or("").to_string(),
            excerpt: strip_html(post.get("excerpt").and_then(|e| e.as_str()).unwrap_or("")),
            image: None,
            title,
        });
    }
    Ok(out)
}

/// News for the installed games ([(uid, region)]), newest first, cached for 15 minutes.
pub fn news(games: &[(String, String)]) -> Vec<NewsItem> {
    let key = games.iter().map(|(u, r)| format!("{u}:{r}")).collect::<Vec<_>>().join(",");
    if let Ok(cache) = NEWS_CACHE.lock() {
        if let Some((at, k, items)) = cache.as_ref() {
            if *k == key && at.elapsed() < NEWS_FRESH {
                return items.clone();
            }
        }
    }
    // Articles first, so a forum post repeating one (same title) is the one dropped.
    let mut items: Vec<NewsItem> = Vec::new();
    for (uid, region) in games {
        items.extend(articles(uid, region).unwrap_or_default());
    }
    for (uid, region) in games {
        for post in forum_news(uid, region).unwrap_or_default() {
            let key = title_key(&post.title);
            if !items.iter().any(|n| title_key(&n.title) == key) {
                items.push(post);
            }
        }
    }
    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.truncate(24);
    if !items.is_empty() {
        if let Ok(mut cache) = NEWS_CACHE.lock() {
            *cache = Some((Instant::now(), key, items.clone()));
        }
    }
    items
}

/// Latest versions for many (product, region) pairs, at most once a minute each.
static VERSION_CACHE: Mutex<Option<HashMap<String, (Instant, (String, String))>>> = Mutex::new(None);

pub fn latest_cached(product: &str, region: &str) -> Result<(String, String), String> {
    let key = format!("{product}:{region}");
    if let Ok(cache) = VERSION_CACHE.lock() {
        if let Some((at, v)) = cache.as_ref().and_then(|c| c.get(&key)) {
            if at.elapsed() < Duration::from_secs(60) {
                return Ok(v.clone());
            }
        }
    }
    let v = latest_version(product, region)?;
    if let Ok(mut cache) = VERSION_CACHE.lock() {
        cache.get_or_insert_with(HashMap::new).insert(key, (Instant::now(), v.clone()));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_to_text() {
        assert_eq!(strip_html("<p>Hotfixes &amp; <b>fixes</b></p>\n  done&hellip;"), "Hotfixes & fixes done\u{2026}");
    }

    #[test]
    fn dates_and_images() {
        assert_eq!(iso_from_ms(1792530000000), "2026-10-20T21:00:00Z");
        assert_eq!(image_url("//bnetcmsus-a.akamaihd.net/cms/a.png").as_deref(), Some("https://bnetcmsus-a.akamaihd.net/cms/a.png"));
        assert_eq!(image_url("https://evil.example/a.png"), None);
    }

    #[test]
    fn overwatch_cards() {
        let page = r#"<a slot="gallery-items" href="/news/1/x" target="_blank"><blz-image slot="media" src="https://bnetcmsus-a.akamaihd.net/cms/t.png"></blz-image><h3 slot="heading">Big &#x27;News&#x27;</h3><blz-timestamp timestamp="2026-09-12T18:10:00.000Z"></blz-timestamp></a>"#;
        let (href, _) = between(page, 0, "<a slot=\"gallery-items\" href=\"", "\"").unwrap();
        assert_eq!(href, "/news/1/x");
        assert_eq!(strip_html(between(page, 0, "<h3 slot=\"heading\">", "</h3>").unwrap().0), "Big 'News'");
    }

    #[test]
    fn news_urls_are_blizzard_forums_only() {
        assert!(is_news_url("https://eu.forums.blizzard.com/en/wow/t/hotfixes/1/1"));
        assert!(!is_news_url("https://example.com/en/wow"));
        assert!(!is_news_url("https://eu.forums.blizzard.com.evil.com/en/x"));
    }
}
