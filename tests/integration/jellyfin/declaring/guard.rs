use regex::Regex;

use lemonfiber_ports::service::{joined, Addressee, Declared, Held, Presentation, ITEM, TOKEN};

pub(super) const OPEN: &str = "0123456789abcdef0123456789abcdef";

pub(super) const CLOSED: &str = "fedcba9876543210fedcba9876543210";

pub(super) const MEMBER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const UPSTREAM: &str = "http://jellyfin:8096";

const PCHAR: &str = "^[A-Za-z0-9._~:@!$&'()*+,;=%/-]*$";

const ABNORMAL: &str = "(?i)(/[.][.]?(/|$)|//|;|%5c)";

const ENCODED_UNRESERVED: &str = "(?i)%(2[5de]|3[0-9]|4[1-9a-f]|5[0-9af]|6[1-9a-f]|7[0-9ae])";

const PLAIN: &str = "^[A-Za-z0-9_.-]*$";

pub(super) fn matching(pattern: &str, text: &str) -> bool {
    Regex::new(pattern).is_ok_and(|compiled| compiled.is_match(text))
}

fn item_in(pattern: &str, text: &str) -> Option<String> {
    let compiled = Regex::new(pattern).ok()?;
    let captured = compiled.captures(text)?;
    captured.name("item").map(|item| item.as_str().to_owned())
}

fn decoded(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        if let (b'%', Some(value)) = (byte, hex) {
            out.push(value);
            at += 3;
        } else {
            out.push(byte);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parameters(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (name.to_owned(), value.to_owned())
        })
        .collect()
}

fn spelled(query: &[(String, String)], names: &[&str]) -> Option<Vec<String>> {
    let mut values = Vec::new();
    for (given, value) in query {
        match names.iter().find(|name| given.eq_ignore_ascii_case(name)) {
            Some(name) if given == name => values.push(value.clone()),
            Some(_) => return None,
            None => {}
        }
    }
    Some(values)
}

fn may(item: &str, read: Option<&str>) -> bool {
    let id: String = item
        .chars()
        .filter(|letter| *letter != '-')
        .collect::<String>()
        .to_ascii_lowercase();
    read == Some(MEMBER) && id == OPEN
}

fn read_token(declared: &Declared, headers: &[(String, String)]) -> Option<String> {
    let token = &declared.token;
    for (name, value) in headers {
        let presented = match value.strip_prefix("Bearer ") {
            Some(bearer) if matching(&token.shape, bearer) => match &token.presented {
                Presentation::Header { value, .. } => value.replace(TOKEN, bearer),
                Presentation::AsIs | Presentation::Query { .. } => value.clone(),
            },
            Some(_) | None => value.clone(),
        };
        let read = if name.eq_ignore_ascii_case("authorization") {
            presented
                .split_once("Token=\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(token, _)| token.to_owned())
        } else {
            Some(presented)
        };
        if read.is_some() {
            return read;
        }
    }
    None
}

pub(super) fn passes(declared: &Declared, target: &str, headers: &[(String, String)]) -> bool {
    let target = target
        .strip_prefix("http://")
        .or_else(|| target.strip_prefix("https://"))
        .map_or(target, |rest| {
            rest.find('/').and_then(|at| rest.get(at..)).unwrap_or("/")
        });
    let (raw, query) = target.split_once('?').unwrap_or((target, ""));
    let path = decoded(raw);
    let query = parameters(query);
    let normal = matching(PCHAR, raw)
        && !matching(ABNORMAL, raw)
        && !matching(ENCODED_UNRESERVED, raw)
        && query.iter().all(|(name, _)| !name.contains('%'));
    if !normal {
        return false;
    }
    let tokens: Vec<&str> = declared
        .token
        .parameters
        .iter()
        .map(String::as_str)
        .collect();
    let query_token = match spelled(&query, &tokens).as_deref() {
        Some([]) => None,
        Some([one]) if matching(&declared.token.shape, one) => Some(one.clone()),
        Some(_) | None => return false,
    };
    let on = |pattern: &str| matching(pattern, raw) || matching(pattern, &path);
    if declared.refused.iter().any(|pattern| on(pattern)) {
        return false;
    }
    let Some(byte_path) = declared.bytes.iter().find(|byte| on(&byte.pattern)) else {
        return true;
    };
    if byte_path.exempt.iter().any(|pattern| on(pattern)) {
        return true;
    }
    if raw.contains('%') || !query.iter().all(|(name, _)| matching(PLAIN, name)) {
        return false;
    }
    let Some(item) =
        item_in(&byte_path.pattern, &path).filter(|item| matching(&byte_path.item, item))
    else {
        return false;
    };
    let mut items = vec![item];
    for named in &byte_path.naming {
        let given = spelled(&query, &[named.parameter.as_str()]);
        match (&named.held, given.as_deref()) {
            (_, Some([])) => {}
            (Held::Asked { shape }, Some([one])) if matching(shape, one) => items.push(one.clone()),
            (Held::Refused | Held::Asked { .. }, _) => return false,
        }
    }
    let Some(question) = &declared.question else {
        return false;
    };
    let read = read_token(declared, headers).or(query_token);
    question.to == Addressee::Upstream
        && items.iter().all(|item| {
            joined(UPSTREAM, &question.path.replace(ITEM, item)).is_some()
                && may(item, read.as_deref())
        })
}
