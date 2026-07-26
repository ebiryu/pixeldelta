//! Signature Version 4, the signing scheme S3 and its compatible services take.
//!
//! The signature covers the method, the path, the query, a fixed set of headers
//! and the hash of the body, so a request cannot be altered in flight without
//! invalidating it.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// Credentials a request is signed with.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub key_id: String,
    pub secret: String,
    /// Set when the credentials come from a role rather than a user.
    pub session_token: Option<String>,
}

/// One request to sign.
pub(crate) struct Request<'a> {
    pub method: &'a str,
    /// Path of the object, already percent-encoded.
    pub path: &'a str,
    pub host: &'a str,
    pub region: &'a str,
    pub body: &'a [u8],
    /// The signing time as `YYYYMMDDTHHMMSSZ`.
    pub timestamp: &'a str,
}

/// Headers to send, including the signature.
pub(crate) fn sign(request: &Request, credentials: &Credentials) -> Vec<(String, String)> {
    let date = &request.timestamp[..8];
    let scope = format!("{date}/{}/s3/aws4_request", request.region);
    let payload_hash = hex(&Sha256::digest(request.body));

    let mut signed_headers = vec![
        ("host", request.host.to_owned()),
        ("x-amz-content-sha256", payload_hash.clone()),
        ("x-amz-date", request.timestamp.to_owned()),
    ];
    if let Some(token) = &credentials.session_token {
        signed_headers.push(("x-amz-security-token", token.clone()));
    }

    let canonical_headers: String = signed_headers
        .iter()
        .map(|(name, value)| format!("{name}:{value}\n"))
        .collect();
    let names: Vec<&str> = signed_headers.iter().map(|(name, _)| *name).collect();
    let signed_names = names.join(";");

    let canonical_request = format!(
        "{}\n{}\n\n{canonical_headers}\n{signed_names}\n{payload_hash}",
        request.method, request.path
    );

    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        request.timestamp,
        hex(&Sha256::digest(canonical_request.as_bytes()))
    );

    let mut key = mac(
        format!("AWS4{}", credentials.secret).as_bytes(),
        date.as_bytes(),
    );
    key = mac(&key, request.region.as_bytes());
    key = mac(&key, b"s3");
    key = mac(&key, b"aws4_request");
    let signature = hex(&mac(&key, string_to_sign.as_bytes()));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_names}, Signature={signature}",
        credentials.key_id
    );

    let mut headers: Vec<(String, String)> = signed_headers
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect();
    headers.push(("authorization".to_owned(), authorization));
    headers
}

/// Percent-encodes a path for the canonical request.
///
/// The slashes between segments stay, since they are part of the path rather
/// than of a segment's name.
pub(crate) fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Formats a point in time as `YYYYMMDDTHHMMSSZ`.
pub(crate) fn timestamp(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let seconds = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60,
    )
}

/// Turns a count of days since 1970-01-01 into a calendar date.
///
/// Shifting the year to start in March puts the leap day at the end of the
/// year, which makes the day of the year a function of the month alone.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn mac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The expected signature comes from a separate implementation of the same
    /// specification, so a slip in either one shows up as a mismatch.
    #[test]
    fn a_signature_matches_the_reference() {
        let credentials = Credentials {
            key_id: "AKIDEXAMPLE".into(),
            secret: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            session_token: None,
        };
        let request = Request {
            method: "PUT",
            path: "/shots/pixeldelta/abc123/images/a.png",
            host: "example.invalid",
            region: "us-east-1",
            body: b"a",
            timestamp: "20260726T120000Z",
        };

        let headers = sign(&request, &credentials);
        let authorization = headers
            .iter()
            .find(|(name, _)| name == "authorization")
            .map(|(_, value)| value.as_str())
            .expect("the headers hold the signature");

        assert_eq!(
            authorization,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20260726/us-east-1/s3/aws4_request, \
             SignedHeaders=host;x-amz-content-sha256;x-amz-date, \
             Signature=5cf44ae8dbd127a19554a0f517dd01e993a23a46799a7754e967e977346821b6"
        );
    }

    #[test]
    fn a_path_keeps_its_slashes_and_encodes_the_rest() {
        assert_eq!(encode_path("/a/b c.png"), "/a/b%20c.png");
        assert_eq!(encode_path("/a/b~c-d_e.png"), "/a/b~c-d_e.png");
    }

    #[test]
    fn a_timestamp_reads_as_the_calendar_date() {
        assert_eq!(timestamp(0), "19700101T000000Z");
        assert_eq!(timestamp(1_769_385_600), "20260126T000000Z");
        // 2024-02-29, a leap day.
        assert_eq!(timestamp(1_709_164_800), "20240229T000000Z");
    }
}
