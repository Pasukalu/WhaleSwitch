use super::{ProbeOutcome, ProbeResult, ProbeStage};
use reqwest::{
    blocking::{Client, ClientBuilder, RequestBuilder},
    dns::{Addrs, Name, Resolve, Resolving},
    header::RETRY_AFTER,
    redirect::Policy,
};
use std::{
    error::Error as StdError,
    fmt,
    io::{self, Read},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use zeroize::Zeroizing;

pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
struct DnsFailure;

impl fmt::Display for DnsFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DNS resolution failed")
    }
}

impl StdError for DnsFailure {}

struct OsResolver;

impl Resolve for OsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let dns_error = || Box::new(DnsFailure) as Box<dyn StdError + Send + Sync>;
            let mut addresses = tokio::net::lookup_host((name.as_str().to_owned(), 0))
                .await.map_err(|_| dns_error())?.peekable();
            if addresses.peek().is_none() {
                return Err(dns_error());
            }
            Ok(Box::new(addresses) as Addrs)
        })
    }
}

pub(super) fn client_builder() -> ClientBuilder {
    Client::builder()
        .no_proxy()
        .use_rustls_tls()
        .redirect(Policy::none())
        .retry(reqwest::retry::never())
        .dns_resolver(Arc::new(OsResolver))
        .connect_timeout(Duration::from_secs(5))
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("WhaleSwitch/", env!("CARGO_PKG_VERSION")))
}

pub(super) fn inspect(request: RequestBuilder) -> ProbeResult {
    let start = Instant::now();
    let report = |stage, outcome, status| ProbeResult {
        elapsed_ms: start.elapsed().as_millis() as u64,
        stage,
        outcome,
        status,
        models: None,
        retry_after_seconds: None,
    };
    let response = match request.send() {
        Ok(response) => response,
        Err(error) => {
            let (stage, outcome) = classify_request_error(&error);
            return report(stage, outcome, None);
        }
    };
    let status = Some(response.status().as_u16());
    if !response.status().is_success() {
        let code = response.status().as_u16();
        let mut result = report(ProbeStage::Http, http_outcome(code), status);
        if matches!(code, 429 | 503) {
            result.retry_after_seconds = response.headers().get(RETRY_AFTER)
                .and_then(|header| header.to_str().ok())
                .and_then(|value| retry_seconds(value, SystemTime::now()));
        }
        return result;
    }
    if response.content_length().is_some_and(|length| length > MAX_RESPONSE_BYTES) {
        return report(ProbeStage::Body, ProbeOutcome::ResponseTooLarge, status);
    }
    let mut body = Zeroizing::new(Vec::new());
    if let Err(error) = response.take(MAX_RESPONSE_BYTES + 1).read_to_end(&mut body) {
        let outcome = if is_timeout(&error) { ProbeOutcome::Timeout } else { ProbeOutcome::BodyError };
        return report(ProbeStage::Body, outcome, status);
    }
    if body.len() as u64 > MAX_RESPONSE_BYTES {
        return report(ProbeStage::Body, ProbeOutcome::ResponseTooLarge, status);
    }
    let value = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(value) => value,
        Err(_) => return report(ProbeStage::Models, ProbeOutcome::InvalidJson, status),
    };
    let Some(data) = value.get("data").and_then(serde_json::Value::as_array) else {
        return report(ProbeStage::Models, ProbeOutcome::UnexpectedResponse, status);
    };
    if !data.iter().all(|entry| entry.get("id").and_then(serde_json::Value::as_str)
        .is_some_and(|id| !id.trim().is_empty())) {
        return report(ProbeStage::Models, ProbeOutcome::UnexpectedResponse, status);
    }
    let mut result = report(ProbeStage::Models, ProbeOutcome::Connected, status);
    result.models = Some(data.len());
    result
}

fn causes(error: &(dyn StdError + 'static)) -> impl Iterator<Item = &(dyn StdError + 'static)> {
    std::iter::successors(Some(error), |cause| {
        let cause = *cause;
        // io::Error::source() can skip the immediately wrapped error.
        cause.downcast_ref::<io::Error>().and_then(io::Error::get_ref)
            .map(|inner| inner as &(dyn StdError + 'static))
            .or_else(|| cause.source())
    }).take(24)
}

fn is_timeout(error: &(dyn StdError + 'static)) -> bool {
    causes(error).any(|cause| {
        cause.downcast_ref::<reqwest::Error>().is_some_and(reqwest::Error::is_timeout)
            || cause.downcast_ref::<io::Error>().is_some_and(|io| io.kind() == io::ErrorKind::TimedOut)
    })
}

fn classify_request_error(error: &reqwest::Error) -> (ProbeStage, ProbeOutcome) {
    let stage = if error.is_connect() { ProbeStage::Connect } else { ProbeStage::Request };
    if is_timeout(error) { return (stage, ProbeOutcome::Timeout); }
    if causes(error).any(|cause| cause.is::<DnsFailure>()) {
        return (ProbeStage::Dns, ProbeOutcome::DnsError);
    }
    if causes(error).any(|cause| cause.is::<rustls::Error>()) {
        return (ProbeStage::Tls, ProbeOutcome::TlsError);
    }
    if causes(error).any(|cause| cause.downcast_ref::<io::Error>()
        .is_some_and(|io| io.kind() == io::ErrorKind::ConnectionRefused)) {
        return (ProbeStage::Connect, ProbeOutcome::ConnectionRefused);
    }
    (stage, ProbeOutcome::NetworkError)
}

fn http_outcome(status: u16) -> ProbeOutcome {
    match status {
        300..=399 => ProbeOutcome::Redirect,
        401 => ProbeOutcome::Rejected,
        403 => ProbeOutcome::Forbidden,
        404 | 405 => ProbeOutcome::Unsupported,
        429 => ProbeOutcome::RateLimited,
        500..=599 => ProbeOutcome::ServerError,
        _ => ProbeOutcome::HttpError,
    }
}

fn retry_seconds(value: &str, now: SystemTime) -> Option<u32> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 { return None; }
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        return value.parse().ok();
    }
    let retry_at = httpdate::parse_http_date(value).ok()?;
    let delay = retry_at.duration_since(now).unwrap_or_default();
    u32::try_from(delay.as_secs() + u64::from(delay.subsec_nanos() > 0)).ok()
}

#[cfg(test)]
mod tests;
