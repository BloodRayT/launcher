//! Новости сообщества из публичного виджета VK: ключ доступа для него не нужен.

use crate::download::read_limited;
use serde::Serialize;

pub const COMMUNITY_WIDGET_URL: &str =
    "https://vk.ru/widget_community.php?gid=48931528&mode=2&no_cover=1";
const VK_BASE_URL: &str = "https://vk.ru";
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const MAX_WIDGET_BYTES: usize = 4 * 1024 * 1024;
const TITLE_LENGTH: usize = 70;
const NEWS_TAG: &str = "VK.RU/FENIXCRAFT";
const EMPTY_POST_TITLE: &str = "Запись сообщества";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NewsPost {
    pub id: u64,
    pub tag: String,
    pub title: String,
    pub text: String,
    pub date: String,
    pub image: String,
    pub images: Vec<String>,
    pub url: String,
}

fn decode_entities(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let decoded = rest.find(';').filter(|end| *end <= 10).and_then(|end| {
            let entity = &rest[1..end];
            let character = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix('#')
                    .and_then(|code| code.parse::<u32>().ok())
                    .and_then(char::from_u32),
            };
            character.map(|character| (character, end))
        });
        match decoded {
            Some((character, end)) => {
                output.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                output.push('&');
                rest = &rest[1..];
            }
        }
    }
    output.push_str(rest);
    output
}

/// Убирает разметку. Ссылка «Показать полностью» удаляется, а скрытое ею продолжение остаётся.
fn html_to_text(html: &str) -> String {
    let mut output = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[..=end];
        if tag.starts_with("<br") {
            output.push('\n');
        }
        // Эмодзи в записях VK заданы картинками, сам символ лежит в alt.
        if tag.starts_with("<img") {
            if let Some(alt) = tag
                .split("alt=\"")
                .nth(1)
                .and_then(|part| part.split('"').next())
            {
                output.push_str(alt);
            }
        }
        rest = &rest[end + 1..];
        let closing = if tag.contains("PostTextMore") {
            Some("</button>")
        } else if tag.contains("wall_post_more") {
            Some("</a>")
        } else {
            None
        };
        if let Some(close) = closing.and_then(|name| Some(rest.find(name)? + name.len())) {
            rest = &rest[close..];
        }
    }
    output.push_str(rest);
    decode_entities(&output).trim().to_string()
}

/// Содержимое блока от открывающего тега с учётом вложенных `<div>`.
fn inner_div(block: &str, marker: &str) -> Option<String> {
    let marker_at = block.find(marker)?;
    let content_start = marker_at + block[marker_at..].find('>')? + 1;
    let content = &block[content_start..];
    let mut depth = 1usize;
    let mut position = 0usize;
    while position < content.len() {
        let next_open = content[position..].find("<div");
        let next_close = content[position..].find("</div>")?;
        match next_open {
            Some(open) if open < next_close => {
                depth += 1;
                position += open + "<div".len();
            }
            _ => {
                depth -= 1;
                if depth == 0 {
                    return Some(content[..position + next_close].to_string());
                }
                position += next_close + "</div>".len();
            }
        }
    }
    None
}

fn background_images(block: &str) -> Vec<String> {
    block
        .split("background-image: url(")
        .skip(1)
        .filter_map(|part| part.split(')').next())
        .map(decode_entities)
        .filter(|url| url.starts_with("https://"))
        .collect()
}

fn post_title(text: &str) -> String {
    let first_line = text.lines().map(str::trim).find(|line| !line.is_empty());
    match first_line {
        Some(line) => line.chars().take(TITLE_LENGTH).collect(),
        None => EMPTY_POST_TITLE.to_string(),
    }
}

fn parse_post(block: &str) -> Option<NewsPost> {
    let date_at = block.find("wcommunity_post_date")?;
    let header = &block[date_at..];
    let href_at = header.find("href=\"")? + "href=\"".len();
    let path = header[href_at..].split('"').next()?;
    let id: u64 = path.rsplit('_').next()?.parse().ok()?;
    let date_start = header.find('>')? + 1;
    let date = html_to_text(&header[date_start..date_start + header[date_start..].find("</a>")?]);
    let text = inner_div(block, "class=\"wall_post_text")
        .map(|html| html_to_text(&html))
        .unwrap_or_default();
    let images = background_images(block);
    Some(NewsPost {
        id,
        tag: NEWS_TAG.to_string(),
        title: post_title(&text),
        text,
        date,
        image: images.first().cloned().unwrap_or_default(),
        images,
        url: format!("{VK_BASE_URL}{path}"),
    })
}

/// Разбирает страницу виджета. Закреплённые записи идут в ней первыми,
/// поэтому порядок задаётся номером записи: новые сначала.
pub fn parse_community_widget(html: &str) -> Vec<NewsPost> {
    let mut posts: Vec<NewsPost> = html
        .split("class=\"wcommunity_post\"")
        .skip(1)
        .filter_map(parse_post)
        .collect();
    posts.sort_by_key(|post| std::cmp::Reverse(post.id));
    posts
}

pub async fn fetch_community_news(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<NewsPost>, String> {
    let response = client
        .get(url)
        .header("User-Agent", BROWSER_USER_AGENT)
        .header("Accept-Language", "ru-RU,ru;q=0.9")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let bytes = read_limited(response, MAX_WIDGET_BYTES).await?;
    // Страница виджета отдаётся в кодировке Windows-1251.
    let (html, _, _) = encoding_rs::WINDOWS_1251.decode(&bytes);
    Ok(parse_community_widget(&html))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDGET: &str = r#"<div id="page_wall_posts"><div class="wcommunity_post">
  <div class="wcommunity_post_header">
    <a class="wcommunity_post_date" href="/wall-48931528_6293" target="_blank">16 октября 2022 в 18:46</a> <span class="wcommunity_explain"><span class="wall_fixed_label"> запись закреплена</span></span>
  </div>
  <div id="wpt-48931528_6293" class="wall_post_cont _wall_post_cont"><div class="wall_post_text" data-testid="wall_post_text">Старая запись <a href="/away.php?to=http%3A%2F%2Ffenixcraft.ru" target="_blank">fenixcraft.ru</a> порт&#33;<br/>Вторая строка</div><div class="page_post_sized_thumbs  clear_fix" style="width: 158px;"><a aria-label="фотография" style="width: 158px; height: 119px;background-image: url(https://sun9-15.vkuserphoto.ru/impg/a.jpg?size=604x453&amp;quality=95)" class="page_post_thumb_wrap"></a><a style="background-image: url(https://sun9-16.vkuserphoto.ru/impg/b.jpg)"></a></div></div>
</div><div class="wcommunity_post">
  <div class="wcommunity_post_header">
    <a class="wcommunity_post_date" href="/wall-48931528_6330" target="_blank">9 мая 2026 в 17:07</a>
  </div>
  <div class="wall_post_cont"><div class="wall_post_text zoom_text">Начало текста<a class="wall_post_more" onclick="x">Показать полностью</a><span style="display: none"> и <div>вложенный</div> конец</span></div></div>
</div><div class="wcommunity_post">
  <div class="wcommunity_post_header"><a class="wcommunity_post_date" href="/wall-48931528_6331">вчера</a></div>
  <div class="wall_post_cont"><div class="page_post_sized_thumbs"><a style="background-image: url(http://insecure.test/x.jpg)"></a></div></div>
</div></div>"#;

    #[test]
    fn posts_are_parsed_newest_first_even_when_an_old_one_is_pinned() {
        let posts = parse_community_widget(WIDGET);

        assert_eq!(
            posts.iter().map(|post| post.id).collect::<Vec<_>>(),
            vec![6331, 6330, 6293]
        );
    }

    #[test]
    fn text_date_link_and_images_are_extracted() {
        let posts = parse_community_widget(WIDGET);
        let post = &posts[2];

        assert_eq!(post.date, "16 октября 2022 в 18:46");
        assert_eq!(post.url, "https://vk.ru/wall-48931528_6293");
        assert_eq!(
            post.text,
            "Старая запись fenixcraft.ru порт!\nВторая строка"
        );
        assert_eq!(post.title, "Старая запись fenixcraft.ru порт!");
        assert_eq!(
            post.images,
            vec![
                "https://sun9-15.vkuserphoto.ru/impg/a.jpg?size=604x453&quality=95".to_string(),
                "https://sun9-16.vkuserphoto.ru/impg/b.jpg".to_string(),
            ]
        );
        assert_eq!(post.image, post.images[0]);
    }

    #[test]
    fn hidden_continuation_is_kept_without_the_show_more_link() {
        let posts = parse_community_widget(WIDGET);

        assert_eq!(posts[1].text, "Начало текста и вложенный конец");
        assert!(posts[1].images.is_empty());
    }

    #[test]
    fn post_without_text_gets_a_title_and_insecure_images_are_dropped() {
        let posts = parse_community_widget(WIDGET);

        assert_eq!(posts[0].title, EMPTY_POST_TITLE);
        assert!(posts[0].images.is_empty());
    }

    #[test]
    fn unknown_markup_gives_no_posts_instead_of_a_failure() {
        assert!(parse_community_widget("<html>captcha</html>").is_empty());
        assert!(parse_community_widget(
            r#"<div class="wcommunity_post"><a class="wcommunity_post_date" href="/x">?</a>"#
        )
        .is_empty());
    }

    #[test]
    fn show_more_button_is_removed_and_emoji_are_kept() {
        let html = r#"Начало<button class="PostTextMore PostTextMore--redesignV3" onclick="hide(this); Wall &amp;&amp; x(&quot;-1_2&quot;)" ><span class="PostTextMore__content">Показать ещё</span></button><span style="display: none"><br><br><img class="emoji" src="/emoji/e/f09f9ba1.png" alt="&#128737;"> На серверах</span>"#;

        assert_eq!(html_to_text(html), "Начало\n\n🛡 На серверах");
    }

    #[test]
    fn entities_and_broken_markup_are_handled() {
        assert_eq!(
            decode_entities("a &amp; b &#33; &unknown; &"),
            "a & b ! &unknown; &"
        );
        assert_eq!(html_to_text("текст <незакрытый"), "текст");
    }
}
