use base64::{Engine, engine::general_purpose::STANDARD};
use image::{
    ImageFormat, ImageReader, Limits, Rgba, RgbaImage,
    imageops::{self, FilterType},
};
use scraper::{Html, Selector};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    io::{Cursor, Read},
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    path::{Path, PathBuf},
    time::Duration,
};
use url::{Host, Url};
use uuid::Uuid;

const MAX_HTML_BYTES: usize = 1_000_000;
const MAX_ICON_BYTES: usize = 5_000_000;
const MAX_MANIFEST_BYTES: usize = 500_000;
const MAX_REDIRECTS: usize = 4;
const MAX_ICON_DIMENSION: u32 = 4_096;
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconOutcome {
    pub icon_path: String,
    pub used_fallback: bool,
    pub warning: Option<String>,
    pub preview_data_url: String,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(default, rename_all = "camelCase")]
struct IconCacheMetadata {
    used_fallback: bool,
    warning: Option<String>,
}

#[derive(Clone, Debug)]
struct IconCandidate {
    url: Url,
    size_hint: u32,
    format_rank: u8,
}

struct FetchedResponse {
    final_url: Url,
    content_type: Option<String>,
    body: Vec<u8>,
}

pub fn retrieve_icon(
    website_url: &str,
    display_name: &str,
    cache_dir: &Path,
    background_color: Option<&str>,
    force_refresh: bool,
) -> Result<IconOutcome, String> {
    let website_url = Url::parse(website_url).map_err(|error| error.to_string())?;
    let background = background_color.map(parse_background_color).transpose()?;
    let cache_path = cache_path(&website_url, display_name, cache_dir, background);
    if cache_path.is_file() && !force_refresh {
        let cached = outcome_from_cache(cache_path.clone())?;
        if !cached.used_fallback {
            return Ok(cached);
        }
    }

    fs::create_dir_all(cache_dir).map_err(|error| error.to_string())?;

    if let Some(provider_url) = favicon_im_url(&website_url)
        && resolve_public_address(&website_url).is_ok()
        && let Ok(response) = fetch_public_url(&provider_url, MAX_ICON_BYTES)
        && let Ok((image, _)) = decode_icon(&response.body, response.content_type.as_deref())
    {
        return save_website_icon(cache_path, image, background);
    }

    let mut candidates = Vec::new();
    let mut manifest_urls = Vec::new();
    let mut final_page_url = None;

    match fetch_public_url(&website_url, MAX_HTML_BYTES) {
        Ok(page) if is_image_content_type(page.content_type.as_deref()) => {
            if let Ok((image, _)) = decode_icon(&page.body, page.content_type.as_deref()) {
                return save_website_icon(cache_path, image, background);
            }
        }
        Ok(page) if is_html_content_type(page.content_type.as_deref()) => {
            final_page_url = Some(page.final_url.clone());
            let (page_candidates, page_manifests) =
                extract_icon_candidates(&page.final_url, &page.body);
            candidates = page_candidates;
            manifest_urls = page_manifests;
        }
        Ok(_) | Err(_) => {}
    }

    for manifest_url in manifest_urls.into_iter().take(2) {
        match fetch_public_url(&manifest_url, MAX_MANIFEST_BYTES) {
            Ok(manifest) => {
                candidates.extend(extract_manifest_candidates(
                    &manifest.final_url,
                    &manifest.body,
                ));
            }
            Err(_) => {}
        }
    }

    candidates.extend(favicon_candidates(final_page_url.as_ref(), &website_url));

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.url.as_str().to_string()));
    candidates.sort_by(|left, right| right.size_hint.cmp(&left.size_hint));

    let mut best: Option<(RgbaImage, u64)> = None;
    for candidate in candidates.into_iter().take(8) {
        let response = match fetch_public_url(&candidate.url, MAX_ICON_BYTES) {
            Ok(response) => response,
            Err(_) => continue,
        };
        match decode_icon(&response.body, response.content_type.as_deref()) {
            Ok((image, rank)) => {
                let score = icon_quality(&image, rank.max(candidate.format_rank));
                if best
                    .as_ref()
                    .is_none_or(|(_, current_score)| score > *current_score)
                {
                    best = Some((image, score));
                }
            }
            Err(_) => {}
        }
    }

    if let Some((image, _)) = best {
        return save_website_icon(cache_path, image, background);
    }

    let fallback = fallback_icon(display_name);
    let fallback_warning = Some(
        "The website icon could not be retrieved. A generated icon is being used.".to_string(),
    );
    save_icon(cache_path, fallback, true, fallback_warning)
}

pub fn preview_from_icon(path: &Path) -> Option<String> {
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let image = reader.decode().ok()?.to_rgba8();
    preview_data_url(&fit_square(&image, 64)).ok()
}

fn cache_path(
    website_url: &Url,
    display_name: &str,
    cache_dir: &Path,
    background: Option<[u8; 3]>,
) -> PathBuf {
    // Older shortcuts still reference their existing ICO paths until refreshed.
    let cache_key = format!(
        "favicon-im-v2\0{}\0{}\0{background:?}",
        website_url.as_str(),
        display_name.trim()
    );
    let digest = Sha256::digest(cache_key.as_bytes());
    cache_dir.join(format!("{}.ico", hex_digest(&digest)))
}

fn parse_background_color(color: &str) -> Result<[u8; 3], String> {
    let Some(hex) = color
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    else {
        return Err("The icon background color must use #RRGGBB format.".to_string());
    };
    let [_, red, green, blue] = u32::from_str_radix(hex, 16)
        .map_err(|error| error.to_string())?
        .to_be_bytes();
    Ok([red, green, blue])
}

fn extract_icon_candidates(page_url: &Url, html: &[u8]) -> (Vec<IconCandidate>, Vec<Url>) {
    let Ok(html) = std::str::from_utf8(html) else {
        return (Vec::new(), Vec::new());
    };
    let document = Html::parse_document(html);
    let Ok(selector) = Selector::parse("link[rel][href]") else {
        return (Vec::new(), Vec::new());
    };
    let mut candidates = Vec::new();
    let mut manifests = Vec::new();

    for element in document.select(&selector) {
        let rel = element
            .value()
            .attr("rel")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let href = element.value().attr("href").unwrap_or_default().trim();
        if href.is_empty() {
            continue;
        }

        if rel
            .split_ascii_whitespace()
            .any(|token| token == "manifest")
        {
            if let Ok(url) = page_url.join(href) {
                manifests.push(url);
            }
            continue;
        }

        let is_icon = rel
            .split_ascii_whitespace()
            .any(|token| token == "icon" || token == "apple-touch-icon");
        if !is_icon {
            continue;
        }

        if let Ok(url) = page_url.join(href) {
            candidates.push(IconCandidate {
                url,
                size_hint: element
                    .value()
                    .attr("sizes")
                    .and_then(parse_size_hint)
                    .unwrap_or_default(),
                format_rank: format_rank(
                    element.value().attr("type"),
                    element.value().attr("href"),
                ),
            });
        }
    }

    (candidates, manifests)
}

fn extract_manifest_candidates(manifest_url: &Url, body: &[u8]) -> Vec<IconCandidate> {
    let Ok(manifest) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let Some(icons) = manifest.get("icons").and_then(Value::as_array) else {
        return Vec::new();
    };

    icons
        .iter()
        .filter_map(|icon| {
            let source = icon.get("src").and_then(Value::as_str)?;
            let url = manifest_url.join(source).ok()?;
            let size_hint = icon
                .get("sizes")
                .and_then(Value::as_str)
                .and_then(parse_size_hint)
                .unwrap_or_default();
            let format = icon.get("type").and_then(Value::as_str);
            Some(IconCandidate {
                format_rank: format_rank(format, Some(source)),
                size_hint,
                url,
            })
        })
        .collect()
}

fn favicon_candidates(page_url: Option<&Url>, website_url: &Url) -> Vec<IconCandidate> {
    let mut urls = Vec::new();
    if let Some(page_url) = page_url {
        if let Ok(url) = page_url.join("favicon.ico") {
            urls.push(url);
        }
    }
    if let Ok(url) = website_url.join("/favicon.ico") {
        urls.push(url);
    }

    urls.into_iter()
        .map(|url| IconCandidate {
            url,
            size_hint: 0,
            format_rank: 1,
        })
        .collect()
}

fn favicon_im_url(website_url: &Url) -> Option<Url> {
    let Host::Domain(hostname) = website_url.host()? else {
        return None;
    };
    let mut url = Url::parse("https://a.favicon.im/").ok()?;
    url.path_segments_mut().ok()?.push(hostname);
    url.query_pairs_mut()
        .append_pair("larger", "true")
        .append_pair("throw-error-on-404", "true");
    Some(url)
}

fn parse_size_hint(sizes: &str) -> Option<u32> {
    sizes
        .split_ascii_whitespace()
        .filter_map(|size| {
            let (width, height) = size.split_once('x')?;
            let width = width.parse::<u32>().ok()?;
            let height = height.parse::<u32>().ok()?;
            Some(width.min(height))
        })
        .max()
}

fn format_rank(content_type: Option<&str>, href: Option<&str>) -> u8 {
    let content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let href = href.unwrap_or_default().to_ascii_lowercase();
    if content_type.contains("png") || href.ends_with(".png") {
        3
    } else if content_type.contains("svg") || href.ends_with(".svg") {
        2
    } else if content_type.contains("icon") || href.ends_with(".ico") {
        1
    } else {
        0
    }
}

fn fetch_public_url(start_url: &Url, max_bytes: usize) -> Result<FetchedResponse, String> {
    let mut current_url = start_url.clone();
    for redirect_number in 0..=MAX_REDIRECTS {
        let pinned_address = resolve_public_address(&current_url)?;
        let host = current_url
            .host_str()
            .ok_or_else(|| "The icon URL has no host.".to_string())?;
        let mut builder = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(8))
            .user_agent("FirefoxShortcutManager/0.1");
        if let Some(address) = pinned_address {
            builder = builder.resolve(host, address);
        }
        let client = builder.build().map_err(|error| error.to_string())?;
        let response = client
            .get(current_url.as_str())
            .send()
            .map_err(|error| error.to_string())?;

        if response.status().is_redirection() {
            if redirect_number == MAX_REDIRECTS {
                return Err("The icon URL redirected too many times.".to_string());
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "The icon redirect did not include a valid location.".to_string())?;
            current_url = current_url
                .join(location)
                .map_err(|error| error.to_string())?;
            validate_fetch_url(&current_url)?;
            continue;
        }

        if !response.status().is_success() {
            return Err(format!(
                "The icon server returned HTTP {}.",
                response.status()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err("The icon response exceeded the size limit.".to_string());
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut body = Vec::new();
        response
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|error| error.to_string())?;
        if body.len() > max_bytes {
            return Err("The icon response exceeded the size limit.".to_string());
        }

        return Ok(FetchedResponse {
            final_url: current_url,
            content_type,
            body,
        });
    }

    Err("The icon URL could not be retrieved.".to_string())
}

fn validate_fetch_url(url: &Url) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Only public HTTP and HTTPS icon URLs are supported.".to_string());
    }
    Ok(())
}

fn resolve_public_address(url: &Url) -> Result<Option<SocketAddr>, String> {
    validate_fetch_url(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| "The icon URL has no host.".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "The icon URL has no valid port.".to_string())?;

    if host.ends_with(".localhost")
        || host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".local")
    {
        return Err("Local network icon URLs are blocked.".to_string());
    }

    if let Ok(address) = host.parse::<IpAddr>() {
        if !is_public_address(address) {
            return Err("Local or reserved network icon URLs are blocked.".to_string());
        }
        return Ok(None);
    }

    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .collect::<Vec<_>>();
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !is_public_address(address.ip()))
    {
        return Err("The icon host resolves to a local or reserved network address.".to_string());
    }

    Ok(addresses.first().copied())
}

fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let octets = address.octets();
            let [first, second, third, _] = octets;
            !(address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_broadcast()
                || address.is_multicast()
                || address.is_unspecified()
                || first == 0
                || (first == 100 && (64..=127).contains(&second))
                || (first == 192 && second == 0 && (third == 0 || third == 2))
                || (first == 192 && second == 88 && third == 99)
                || (first == 198 && (second == 18 || second == 19 || second == 51))
                || (first == 203 && second == 0 && third == 113)
                || first >= 224)
        }
        IpAddr::V6(address) => {
            let octets = address.octets();
            let mapped_v4 = if octets[..10].iter().all(|byte| *byte == 0)
                && octets[10] == 0xff
                && octets[11] == 0xff
            {
                Some(IpAddr::V4(std::net::Ipv4Addr::new(
                    octets[12], octets[13], octets[14], octets[15],
                )))
            } else {
                None
            };
            if let Some(mapped_v4) = mapped_v4 {
                return is_public_address(mapped_v4);
            }

            let first_segment = address.segments()[0];
            !(address.is_loopback()
                || address.is_unspecified()
                || address.is_multicast()
                || (first_segment & 0xfe00) == 0xfc00
                || (first_segment & 0xffc0) == 0xfe80
                || (address.segments()[0] == 0x2001 && address.segments()[1] == 0x0db8))
        }
    }
}

fn is_html_content_type(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| value.to_ascii_lowercase().contains("text/html"))
}

fn is_image_content_type(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| value.to_ascii_lowercase().starts_with("image/"))
}

fn decode_icon(bytes: &[u8], content_type: Option<&str>) -> Result<(RgbaImage, u8), String> {
    if content_type.is_some_and(|value| value.to_ascii_lowercase().contains("svg"))
        || looks_like_svg(bytes)
    {
        return render_svg(bytes).map(|image| (image, 2));
    }

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_ICON_DIMENSION);
    limits.max_image_height = Some(MAX_ICON_DIMENSION);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);

    let decoded = reader.decode().map_err(|error| error.to_string())?;
    let (width, height) = (decoded.width(), decoded.height());
    if width == 0 || height == 0 || width > MAX_ICON_DIMENSION || height > MAX_ICON_DIMENSION {
        return Err("The website icon dimensions are not supported.".to_string());
    }
    Ok((decoded.to_rgba8(), 3))
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
    sample.contains("<svg")
}

fn render_svg(bytes: &[u8]) -> Result<RgbaImage, String> {
    if bytes.len() > 1_000_000 {
        return Err("The SVG icon exceeded the size limit.".to_string());
    }
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options).map_err(|error| error.to_string())?;
    let size = tree.size();
    let (width, height) = (size.width(), size.height());
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > MAX_ICON_DIMENSION as f32
        || height > MAX_ICON_DIMENSION as f32
    {
        return Err("The SVG icon dimensions are not supported.".to_string());
    }

    let output_size = 256_u32;
    let scale = (output_size as f32 / width).min(output_size as f32 / height);
    let offset_x = (output_size as f32 - width * scale) / 2.0;
    let offset_y = (output_size as f32 - height * scale) / 2.0;
    let transform =
        resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(output_size, output_size)
        .ok_or_else(|| "The SVG icon could not be rendered.".to_string())?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    RgbaImage::from_raw(output_size, output_size, pixmap.data().to_vec())
        .ok_or_else(|| "The SVG icon could not be converted.".to_string())
}

fn icon_quality(image: &RgbaImage, format_rank: u8) -> u64 {
    let smallest_dimension = image.width().min(image.height()) as u64;
    let largest_dimension = image.width().max(image.height()) as u64;
    let square_penalty = largest_dimension.saturating_sub(smallest_dimension);
    smallest_dimension * 10_000 + format_rank as u64 * 100 - square_penalty.min(9_999)
}

fn fallback_icon(name: &str) -> RgbaImage {
    let digest = Sha256::digest(name.as_bytes());
    let background = Rgba([
        72 + digest[0] % 128,
        72 + digest[1] % 128,
        72 + digest[2] % 128,
        255,
    ]);
    let mut image = RgbaImage::from_pixel(256, 256, background);
    let initial = name
        .chars()
        .find(|character| character.is_ascii_alphanumeric())
        .unwrap_or('F')
        .to_ascii_uppercase();
    let glyph = glyph(initial);
    let cell_size = 24_u32;
    let width = 5 * cell_size;
    let height = 7 * cell_size;
    let left = (256 - width) / 2;
    let top = (256 - height) / 2;

    for (row, bits) in glyph.into_iter().enumerate() {
        for column in 0..5 {
            if bits & (1 << (4 - column)) == 0 {
                continue;
            }
            for y in 0..cell_size {
                for x in 0..cell_size {
                    image.put_pixel(
                        left + column * cell_size + x,
                        top + row as u32 * cell_size + y,
                        Rgba([255, 255, 255, 255]),
                    );
                }
            }
        }
    }

    image
}

fn glyph(character: char) -> [u8; 7] {
    match character {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        _ => [31, 17, 1, 2, 4, 0, 4],
    }
}

fn save_website_icon(
    path: PathBuf,
    mut image: RgbaImage,
    background: Option<[u8; 3]>,
) -> Result<IconOutcome, String> {
    if let Some(background) = background {
        remove_edge_background(&mut image, background);
    }
    save_icon(path, trim_transparent_edges(image), false, None)
}

fn remove_edge_background(image: &mut RgbaImage, background: [u8; 3]) {
    const CHANNEL_TOLERANCE: u8 = 12;

    let (width, height) = (image.width() as usize, image.height() as usize);
    if width == 0 || height == 0 {
        return;
    }

    let pixels = image.as_raw();
    let mut exterior = vec![false; width * height];
    let mut pending = VecDeque::new();
    let visit =
        |x: usize, y: usize, exterior: &mut [bool], pending: &mut VecDeque<(usize, usize)>| {
            let index = y * width + x;
            let pixel = &pixels[index * 4..][..4];
            if !exterior[index]
                && (pixel[3] == 0
                    || pixel[..3]
                        .iter()
                        .zip(background)
                        .all(|(&actual, expected)| actual.abs_diff(expected) <= CHANNEL_TOLERANCE))
            {
                exterior[index] = true;
                pending.push_back((x, y));
            }
        };

    for x in 0..width {
        visit(x, 0, &mut exterior, &mut pending);
        visit(x, height - 1, &mut exterior, &mut pending);
    }
    for y in 0..height {
        visit(0, y, &mut exterior, &mut pending);
        visit(width - 1, y, &mut exterior, &mut pending);
    }
    while let Some((x, y)) = pending.pop_front() {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(next_x) = x.checked_add_signed(dx).filter(|&x| x < width)
                && let Some(next_y) = y.checked_add_signed(dy).filter(|&y| y < height)
            {
                visit(next_x, next_y, &mut exterior, &mut pending);
            }
        }
    }

    // Keep the source if the chosen color would erase all visible artwork.
    if !image
        .pixels()
        .zip(&exterior)
        .any(|(pixel, &outside)| pixel[3] != 0 && !outside)
    {
        return;
    }
    for (pixel, outside) in image.pixels_mut().zip(exterior) {
        if outside {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }
}

fn trim_transparent_edges(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (x, y, _) in image
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel[3] != 0)
    {
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
    }
    if left >= right || top >= bottom || (left, top, right, bottom) == (0, 0, width, height) {
        return image;
    }
    imageops::crop_imm(&image, left, top, right - left, bottom - top).to_image()
}

fn save_icon(
    path: PathBuf,
    image: RgbaImage,
    used_fallback: bool,
    warning: Option<String>,
) -> Result<IconOutcome, String> {
    let ico = encode_multires_ico(&image)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Icon cache path is invalid.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temp_path = parent.join(format!(".fsm-{}.tmp", Uuid::new_v4()));
    if let Err(error) = fs::write(&temp_path, ico) {
        let _ = fs::remove_file(&temp_path);
        return Err(error.to_string());
    }
    if let Err(error) = replace_cached_icon(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = save_cache_metadata(
        &path,
        &IconCacheMetadata {
            used_fallback,
            warning: warning.clone(),
        },
    ) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }

    Ok(IconOutcome {
        icon_path: path.to_string_lossy().into_owned(),
        used_fallback,
        warning,
        preview_data_url: preview_data_url(&fit_square(&image, 64))?,
    })
}

fn replace_cached_icon(temp_path: &Path, path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            Win32::Storage::FileSystem::{
                MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
            },
            core::PCWSTR,
        };

        let temp_wide = temp_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let path_wide = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        unsafe {
            MoveFileExW(
                PCWSTR(temp_wide.as_ptr()),
                PCWSTR(path_wide.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|error| error.to_string())
    }

    #[cfg(not(windows))]
    {
        fs::rename(temp_path, path).map_err(|error| error.to_string())
    }
}

fn outcome_from_cache(path: PathBuf) -> Result<IconOutcome, String> {
    let metadata = load_cache_metadata(&path)?;
    let preview_data_url =
        preview_from_icon(&path).ok_or_else(|| "The cached icon could not be read.".to_string())?;
    Ok(IconOutcome {
        icon_path: path.to_string_lossy().into_owned(),
        used_fallback: metadata.used_fallback,
        warning: metadata.warning,
        preview_data_url,
    })
}

fn load_cache_metadata(path: &Path) -> Result<IconCacheMetadata, String> {
    match fs::read(path.with_extension("json")) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|error| format!("Icon cache metadata is invalid: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(IconCacheMetadata::default())
        }
        Err(error) => Err(format!("Icon cache metadata could not be read: {error}")),
    }
}

fn save_cache_metadata(path: &Path, metadata: &IconCacheMetadata) -> Result<(), String> {
    let encoded = serde_json::to_vec(metadata)
        .map_err(|error| format!("Icon cache metadata could not be encoded: {error}"))?;
    fs::write(path.with_extension("json"), encoded)
        .map_err(|error| format!("Icon cache metadata could not be saved: {error}"))
}

fn encode_multires_ico(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut png_images = Vec::new();
    for size in ICO_SIZES {
        let square = fit_square(image, size);
        let mut cursor = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(square)
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        png_images.push(cursor.into_inner());
    }

    let directory_size = 6 + 16 * png_images.len();
    let mut output =
        Vec::with_capacity(directory_size + png_images.iter().map(Vec::len).sum::<usize>());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&(png_images.len() as u16).to_le_bytes());

    let mut image_offset = directory_size as u32;
    for (size, png) in ICO_SIZES.iter().zip(&png_images) {
        let dimension = if *size == 256 { 0 } else { *size as u8 };
        output.push(dimension);
        output.push(dimension);
        output.push(0);
        output.push(0);
        output.extend_from_slice(&1_u16.to_le_bytes());
        output.extend_from_slice(&32_u16.to_le_bytes());
        output.extend_from_slice(&(png.len() as u32).to_le_bytes());
        output.extend_from_slice(&image_offset.to_le_bytes());
        image_offset += png.len() as u32;
    }

    for png in png_images {
        output.extend_from_slice(&png);
    }

    Ok(output)
}

fn fit_square(image: &RgbaImage, size: u32) -> RgbaImage {
    let width = image.width().max(1);
    let height = image.height().max(1);
    let scale = (size as f64 / width as f64).min(size as f64 / height as f64);
    let resized_width = ((width as f64 * scale).round() as u32).clamp(1, size);
    let resized_height = ((height as f64 * scale).round() as u32).clamp(1, size);
    let resized = imageops::resize(image, resized_width, resized_height, FilterType::Lanczos3);
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let left = (size - resized_width) / 2;
    let top = (size - resized_height) / 2;
    imageops::overlay(&mut canvas, &resized, left.into(), top.into());
    canvas
}

fn preview_data_url(image: &RgbaImage) -> Result<String, String> {
    let mut cursor = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(cursor.into_inner())
    ))
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_local_and_reserved_addresses() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.20.1.1",
            "192.168.1.1",
            "169.254.1.1",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert!(!is_public_address(address.parse().unwrap()), "{address}");
        }
    }

    #[test]
    fn permits_public_addresses() {
        assert!(is_public_address("1.1.1.1".parse().unwrap()));
        assert!(is_public_address("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn generates_a_multiresolution_ico_with_alpha() {
        let mut image = RgbaImage::from_pixel(256, 256, Rgba([10, 20, 30, 255]));
        image.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        let ico = encode_multires_ico(&image).unwrap();

        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), ICO_SIZES.len() as u16);
        assert_eq!(ico[6], 16);
        assert_eq!(ico[102], 0);
    }

    #[test]
    fn removes_only_edge_connected_background_and_centers_the_trimmed_artwork() {
        let mut image = RgbaImage::new(10, 12);
        let background = Rgba([240, 225, 215, 255]);
        let logo = Rgba([10, 20, 30, 255]);
        for y in 2..10 {
            for x in 1..9 {
                image.put_pixel(x, y, background);
            }
        }
        for y in 3..9 {
            for x in 3..7 {
                image.put_pixel(x, y, logo);
            }
        }
        image.put_pixel(4, 5, background);

        remove_edge_background(&mut image, [240, 225, 215]);
        let trimmed = trim_transparent_edges(image);
        assert_eq!(trimmed.dimensions(), (4, 6));
        assert_eq!(*trimmed.get_pixel(1, 2), background);

        let square = fit_square(&trimmed, 6);
        assert_eq!(square.dimensions(), (6, 6));
        assert_eq!(square.get_pixel(0, 0)[3], 0);
        assert_eq!(square.get_pixel(5, 5)[3], 0);
        assert_eq!(*square.get_pixel(1, 0), logo);
        assert_eq!(*square.get_pixel(4, 5), logo);
    }

    #[test]
    fn keeps_artwork_when_background_removal_would_erase_it() {
        let mut image = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 255]));
        let original = image.clone();
        remove_edge_background(&mut image, [255, 255, 255]);
        assert_eq!(image, original);
    }

    #[test]
    fn fallback_icons_are_stable_for_the_same_name() {
        assert_eq!(fallback_icon("Teams"), fallback_icon("Teams"));
        assert_ne!(fallback_icon("Teams"), fallback_icon("Outlook"));
    }

    #[test]
    fn cached_fallback_icons_keep_their_warning() {
        let cache_dir = std::env::temp_dir().join(format!("fsm-icon-cache-{}", Uuid::new_v4()));
        let cache_path = cache_dir.join("fallback.ico");
        let warning = "A generated icon is being used.".to_string();
        let saved = save_icon(
            cache_path,
            fallback_icon("Teams"),
            true,
            Some(warning.clone()),
        )
        .unwrap();
        let cached = outcome_from_cache(PathBuf::from(saved.icon_path)).unwrap();

        assert!(cached.used_fallback);
        assert_eq!(cached.warning.as_deref(), Some(warning.as_str()));
        let _ = fs::remove_dir_all(cache_dir);
    }

    #[test]
    fn refreshed_icons_replace_cached_fallback_metadata() {
        let cache_dir = std::env::temp_dir().join(format!("fsm-icon-cache-{}", Uuid::new_v4()));
        let cache_path = cache_dir.join("fallback.ico");
        save_icon(
            cache_path.clone(),
            fallback_icon("Outlook"),
            true,
            Some("The website icon could not be retrieved.".to_string()),
        )
        .unwrap();
        save_icon(
            cache_path.clone(),
            RgbaImage::from_pixel(256, 256, Rgba([12, 34, 56, 255])),
            false,
            None,
        )
        .unwrap();

        let cached = outcome_from_cache(cache_path).unwrap();
        assert!(!cached.used_fallback);
        assert!(cached.warning.is_none());
        let _ = fs::remove_dir_all(cache_dir);
    }

    #[test]
    fn discovers_relative_icons_and_manifest_links() {
        let page_url = Url::parse("https://example.com/apps/").unwrap();
        let html = br#"
          <html><head>
            <link rel="icon" sizes="64x64" href="../icons/site.png">
            <link rel="manifest" href="/manifest.webmanifest">
          </head></html>
        "#;
        let (icons, manifests) = extract_icon_candidates(&page_url, html);

        assert_eq!(icons[0].url.as_str(), "https://example.com/icons/site.png");
        assert_eq!(icons[0].size_hint, 64);
        assert_eq!(
            manifests[0].as_str(),
            "https://example.com/manifest.webmanifest"
        );
    }

    #[test]
    fn tries_favicon_relative_to_the_final_page_url() {
        let website_url = Url::parse("https://outlook.cloud.microsoft/").unwrap();
        let final_page_url = Url::parse("https://outlook.cloud.microsoft/mail/").unwrap();

        let candidates = favicon_candidates(Some(&final_page_url), &website_url);

        assert_eq!(
            candidates[0].url.as_str(),
            "https://outlook.cloud.microsoft/mail/favicon.ico"
        );
        assert_eq!(
            candidates[1].url.as_str(),
            "https://outlook.cloud.microsoft/favicon.ico"
        );
    }

    #[test]
    fn requests_a_large_favicon_using_only_the_website_hostname() {
        let website_url =
            Url::parse("https://outlook.cloud.microsoft:8443/mail/?view=inbox#folder").unwrap();

        let favicon_url = favicon_im_url(&website_url).unwrap();

        assert_eq!(
            favicon_url.as_str(),
            "https://a.favicon.im/outlook.cloud.microsoft?larger=true&throw-error-on-404=true"
        );
    }

    #[test]
    fn rejects_non_web_icon_sources() {
        assert!(validate_fetch_url(&Url::parse("file:///C:/icon.png").unwrap()).is_err());
        assert!(validate_fetch_url(&Url::parse("data:image/png,abc").unwrap()).is_err());
        assert!(validate_fetch_url(&Url::parse("https://example.com/icon.png").unwrap()).is_ok());
    }
}
