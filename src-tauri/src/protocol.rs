use crate::errors::AppError;
use url::{Url, form_urlencoded};

const PROTOCOL_PREFIX: &str = "ext+container:";
const MAX_WEBSITE_URL_LENGTH: usize = 8_192;

pub fn validate_website_url(input: &str) -> Result<String, AppError> {
    let input = input.trim();
    let parsed = Url::parse(input).map_err(|error| {
        AppError::new(
            "invalid_url",
            "Enter a valid website URL beginning with http:// or https://.",
            Some(error.to_string()),
        )
    })?;

    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(AppError::new(
            "invalid_url",
            "Only HTTP and HTTPS website URLs without embedded credentials are supported.",
            None,
        ));
    }

    let normalized = parsed.to_string();
    if normalized.len() > MAX_WEBSITE_URL_LENGTH {
        return Err(AppError::new(
            "invalid_url",
            "The website URL is too long to use in a Firefox shortcut.",
            None,
        ));
    }

    Ok(normalized)
}

pub fn generate_container_url(
    container_id: &str,
    container_name: &str,
    website_url: &str,
) -> Result<String, AppError> {
    if container_id.trim().is_empty() || container_name.trim().is_empty() {
        return Err(AppError::new(
            "invalid_container",
            "Select an existing Firefox container before creating a shortcut.",
            None,
        ));
    }

    let website_url = validate_website_url(website_url)?;
    let query = [
        ("id", container_id),
        ("name", container_name),
        ("url", website_url.as_str()),
    ]
    .into_iter()
    .map(|(key, value)| format!("{key}={}", encode_handler_value(value)))
    .collect::<Vec<_>>()
    .join("&");

    Ok(format!("{PROTOCOL_PREFIX}{query}"))
}

fn encode_handler_value(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_component(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0;

        while index < bytes.len() {
            if bytes[index] == b'%' && index + 2 < bytes.len() {
                let high = (bytes[index + 1] as char).to_digit(16).unwrap();
                let low = (bytes[index + 2] as char).to_digit(16).unwrap();
                decoded.push(((high << 4) | low) as u8);
                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }

        String::from_utf8(decoded).unwrap()
    }

    fn extension_parameters(protocol_url: &str) -> Vec<(String, String)> {
        let browser_encoded =
            form_urlencoded::byte_serialize(protocol_url.as_bytes()).collect::<String>();
        let handler_url = decode_component(&browser_encoded);
        let query = handler_url.strip_prefix(PROTOCOL_PREFIX).unwrap();
        form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect()
    }

    #[test]
    fn validates_http_and_https_urls_only() {
        assert!(validate_website_url("https://example.com").is_ok());
        assert!(validate_website_url("http://example.com/path").is_ok());
        assert!(validate_website_url("file:///C:/secret").is_err());
        assert!(validate_website_url("javascript:alert(1)").is_err());
        assert!(validate_website_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn preserves_container_names_and_urls_through_handler_decoding() {
        let name = "Work & Équipe + 100%";
        let url = "https://example.com/path?q=a%26b&next=%E2%9C%93#section";
        let generated = generate_container_url("firefox-container-2", name, url).unwrap();
        let parsed = extension_parameters(&generated);

        assert_eq!(
            parsed,
            vec![
                ("id".to_string(), "firefox-container-2".to_string()),
                ("name".to_string(), name.to_string()),
                ("url".to_string(), validate_website_url(url).unwrap()),
            ]
        );
    }

    #[test]
    fn extension_receives_a_parseable_website_url() {
        let generated = generate_container_url(
            "firefox-container-1",
            "Outlook",
            "https://outlook.cloud.microsoft/",
        )
        .unwrap();
        let parsed = extension_parameters(&generated);

        assert_eq!(
            parsed.iter().find(|(key, _)| key == "url").unwrap().1,
            "https://outlook.cloud.microsoft/"
        );
    }
}
