use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

use brotli::Decompressor;
use encoding_rs::{Encoding, UTF_8};
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};

const MAX_DECODED_BODY_BYTES: u64 = 10 * 1024 * 1024;

pub fn decode_body_text(body: &[u8], headers: &BTreeMap<String, String>) -> Option<String> {
    if body.is_empty() {
        return Some(String::new());
    }

    let content_type = header(headers, "content-type");
    if !is_text_content_type(content_type) {
        return None;
    }

    let decoded = decode_content_encoding(body, header(headers, "content-encoding"))?;
    let charset = content_type
        .and_then(parse_charset)
        .and_then(|label| Encoding::for_label(label.as_bytes()))
        .unwrap_or(UTF_8);
    let (text, _, had_errors) = charset.decode(&decoded);
    (!had_errors).then(|| text.into_owned())
}

fn decode_content_encoding(body: &[u8], content_encoding: Option<&str>) -> Option<Vec<u8>> {
    let mut decoded = body.to_vec();
    let encodings = content_encoding
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    for encoding in encodings.into_iter().rev() {
        decoded = match encoding.to_ascii_lowercase().as_str() {
            "identity" => decoded,
            "gzip" | "x-gzip" => read_limited(GzDecoder::new(Cursor::new(decoded)))?,
            "deflate" => decode_deflate(&decoded)?,
            "br" => read_limited(Decompressor::new(Cursor::new(decoded), 4096))?,
            _ => return None,
        };
    }
    Some(decoded)
}

fn decode_deflate(body: &[u8]) -> Option<Vec<u8>> {
    read_limited(ZlibDecoder::new(Cursor::new(body)))
        .or_else(|| read_limited(DeflateDecoder::new(Cursor::new(body))))
}

fn read_limited(reader: impl Read) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    reader
        .take(MAX_DECODED_BODY_BYTES + 1)
        .read_to_end(&mut output)
        .ok()?;
    (output.len() as u64 <= MAX_DECODED_BODY_BYTES).then_some(output)
}

fn header<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn parse_charset(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.trim().split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['\'', '"']))
    })
}

fn is_text_content_type(content_type: Option<&str>) -> bool {
    let Some(content_type) = content_type else {
        return true;
    };
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    media_type.starts_with("text/")
        || media_type == "application/json"
        || media_type.ends_with("+json")
        || media_type == "application/xml"
        || media_type.ends_with("+xml")
        || media_type == "application/javascript"
        || media_type == "application/x-javascript"
        || media_type == "application/x-www-form-urlencoded"
        || media_type == "application/graphql"
}
