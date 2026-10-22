//! Finds the video-call link in a calendar event (Zoom, Microsoft Teams, Google Meet, Webex).
//! Invite text is untrusted, so only https links on the providers' own hosts are accepted, and
//! only those are ever opened.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Zoom,
    Teams,
    Meet,
    Webex,
}

impl Provider {
    pub fn label(self) -> &'static str {
        match self {
            Provider::Zoom => "Zoom",
            Provider::Teams => "Microsoft Teams",
            Provider::Meet => "Google Meet",
            Provider::Webex => "Webex",
        }
    }

    /// Apps that hold the microphone during this provider's calls. Browsers count for every
    /// provider, since all four also run on the web.
    pub fn runs_in(self, bundle_id: &str) -> bool {
        const BROWSERS: &[&str] = &[
            "com.google.Chrome",
            "com.apple.Safari",
            "company.thebrowser.Browser",
            "org.mozilla.firefox",
            "com.microsoft.edgemac",
            "com.brave.Browser",
        ];
        if BROWSERS.contains(&bundle_id) {
            return true;
        }
        match self {
            Provider::Zoom => bundle_id == "us.zoom.xos",
            Provider::Teams => {
                bundle_id == "com.microsoft.teams2" || bundle_id == "com.microsoft.teams"
            }
            Provider::Meet => false,
            Provider::Webex => {
                bundle_id == "com.cisco.webexmeetingsapp" || bundle_id == "Cisco-Systems.Spark"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallLink {
    pub provider: Provider,
    pub url: String,
}

/// The first call link in the event's URL field, then its location, then its notes.
pub fn find(url: &str, location: &str, notes: &str) -> Option<CallLink> {
    [url, location, notes].into_iter().find_map(find_in)
}

/// The first call link anywhere in a piece of text.
pub fn find_in(text: &str) -> Option<CallLink> {
    text.split(|c: char| {
        c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '(' | ')' | '[' | ']')
    })
    .filter(|w| w.contains("://") || w.starts_with("www."))
    .find_map(|w| {
        classify(&unwrap_redirect(
            w.trim_end_matches(['.', ',', ';', ':', '!', '?']),
        ))
    })
}

/// Outlook "safe links" and Google redirects wrap the real address in a query parameter.
fn unwrap_redirect(raw: &str) -> String {
    let Some((host, _)) = split_url(raw) else {
        return raw.to_string();
    };
    let param = if host.ends_with("safelinks.protection.outlook.com") {
        "url"
    } else if host == "www.google.com" || host == "google.com" {
        "q"
    } else {
        return raw.to_string();
    };
    query_param(raw, param).unwrap_or_else(|| raw.to_string())
}

fn query_param(url: &str, name: &str) -> Option<String> {
    let query = url.split_once('?')?.1.split('#').next()?;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then(|| percent_decode(v))
    })
}

fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi << 4 | lo);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Lower-cased host and the path (without query), for an https URL.
fn split_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].to_ascii_lowercase();
    if host.is_empty() || host.contains('@') || host.contains(':') {
        return None;
    }
    let path = rest[end..]
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .to_string();
    Some((host, path))
}

fn on_domain(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Recognises a join link by host and path, so a provider's marketing page or a recording link
/// doesn't count.
pub fn classify(url: &str) -> Option<CallLink> {
    let (host, path) = split_url(url)?;
    let provider = if (on_domain(&host, "zoom.us") || on_domain(&host, "zoomgov.com"))
        && (path.starts_with("/j/") || path.starts_with("/my/") || path.starts_with("/w/"))
    {
        Provider::Zoom
    } else if (host == "teams.microsoft.com" && path.starts_with("/l/meetup-join/"))
        || (host == "teams.live.com" && path.starts_with("/meet/"))
    {
        Provider::Teams
    } else if host == "meet.google.com"
        && path.len() > 1
        && path[1..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        Provider::Meet
    } else if on_domain(&host, "webex.com")
        && (path.starts_with("/meet/") || path.starts_with("/join/") || path.contains("j.php"))
    {
        Provider::Webex
    } else {
        return None;
    };
    Some(CallLink {
        provider,
        url: url.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(text: &str) -> Option<Provider> {
        find_in(text).map(|l| l.provider)
    }

    #[test]
    fn finds_each_provider_in_real_invite_text() {
        let zoom = "Join Zoom Meeting\nhttps://us02web.zoom.us/j/85512345678?pwd=abcDEF123.1\n\nMeeting ID: 855 1234 5678";
        assert_eq!(
            find_in(zoom).unwrap(),
            CallLink {
                provider: Provider::Zoom,
                url: "https://us02web.zoom.us/j/85512345678?pwd=abcDEF123.1".into()
            }
        );
        assert_eq!(
            provider("Personal room: https://zoom.us/my/priya.shah"),
            Some(Provider::Zoom)
        );
        assert_eq!(
            provider(
                "Microsoft Teams meeting\nJoin: <https://teams.microsoft.com/l/meetup-join/19%3ameeting_ABC%40thread.v2/0?context=%7b%22Tid%22%7d>"
            ),
            Some(Provider::Teams)
        );
        assert_eq!(
            provider("https://teams.live.com/meet/9876543210"),
            Some(Provider::Teams)
        );
        assert_eq!(
            provider(
                "Google Meet joining info\nVideo call link: https://meet.google.com/abc-defg-hij."
            ),
            Some(Provider::Meet)
        );
        assert_eq!(
            provider("https://acme.webex.com/acme/j.php?MTID=m1234"),
            Some(Provider::Webex)
        );
        assert_eq!(
            provider("https://acme.webex.com/meet/priya"),
            Some(Provider::Webex)
        );
    }

    #[test]
    fn unwraps_outlook_safe_links_and_google_redirects() {
        let safe = "https://nam12.safelinks.protection.outlook.com/?url=https%3A%2F%2Fus06web.zoom.us%2Fj%2F8812345%3Fpwd%3Dxyz&data=05%7C02&reserved=0";
        assert_eq!(
            find_in(safe).unwrap().url,
            "https://us06web.zoom.us/j/8812345?pwd=xyz"
        );
        let google = "https://www.google.com/url?q=https://meet.google.com/xyz-abcd-efg&sa=D&source=calendar";
        assert_eq!(
            find_in(google).unwrap().url,
            "https://meet.google.com/xyz-abcd-efg"
        );
    }

    #[test]
    fn prefers_the_url_field_then_location_then_notes() {
        let l = find(
            "",
            "https://meet.google.com/aaa-bbbb-ccc",
            "Backup: https://zoom.us/j/1",
        )
        .unwrap();
        assert_eq!(l.provider, Provider::Meet);
        assert_eq!(
            find("", "Room 4", "https://zoom.us/j/1").unwrap().provider,
            Provider::Zoom
        );
        assert_eq!(find("", "Room 4", "No call today"), None);
    }

    #[test]
    fn rejects_lookalikes_other_schemes_and_non_join_pages() {
        assert_eq!(provider("https://zoom.us.evil.example/j/123"), None);
        assert_eq!(provider("https://evilzoom.us/j/123"), None);
        assert_eq!(provider("http://zoom.us/j/123"), None);
        assert_eq!(provider("https://user@zoom.us/j/123"), None);
        assert_eq!(provider("https://zoom.us/pricing"), None);
        assert_eq!(provider("https://zoom.us/rec/share/abc"), None);
        assert_eq!(provider("https://meet.google.com/"), None);
        assert_eq!(provider("https://teams.microsoft.com/v2/"), None);
        assert_eq!(provider("file:///Applications/zoom.us.app"), None);
    }

    #[test]
    fn matches_providers_to_the_apps_that_run_their_calls() {
        assert!(Provider::Zoom.runs_in("us.zoom.xos"));
        assert!(Provider::Zoom.runs_in("com.google.Chrome"));
        assert!(!Provider::Zoom.runs_in("com.microsoft.teams2"));
        assert!(Provider::Teams.runs_in("com.microsoft.teams2"));
        assert!(Provider::Meet.runs_in("com.apple.Safari"));
        assert!(!Provider::Meet.runs_in("us.zoom.xos"));
    }

    #[test]
    fn decodes_percent_escapes_and_keeps_bad_ones() {
        assert_eq!(percent_decode("a%2Fb%3Fc"), "a/b?c");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }
}
