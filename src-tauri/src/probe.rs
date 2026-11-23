use crate::{Engine, Error, Result, Tool};
use reqwest::{blocking::Client, header::{HeaderValue, AUTHORIZATION}, redirect::Policy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr}, time::{Duration, Instant}};
use url::{Host, Url};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol { Openai, Anthropic }

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeInput {
    pub tool: Tool,
    pub protocol: Protocol,
    pub endpoint: String,
    #[serde(default)]
    pub anonymous: bool,
}

#[derive(Serialize)]
pub struct ProbeResult {
    pub elapsed_ms: u64,
    pub status: Option<u16>,
    pub outcome: &'static str,
    pub models: Option<usize>,
}

impl ProbeInput {
    fn target(&self) -> Result<Url> {
        let mut url = Url::parse(&self.endpoint).map_err(|_| Error::InvalidEndpoint)?;
        if self.endpoint.len() > 512 || self.endpoint.contains('%') || self.endpoint.chars().any(char::is_whitespace)
            || !url.username().is_empty() || url.password().is_some()
            || url.query().is_some() || url.fragment().is_some() {
            return Err(Error::InvalidEndpoint);
        }
        let local = match url.host().ok_or(Error::InvalidEndpoint)? {
            Host::Domain(host) => host == "localhost",
            Host::Ipv4(ip) => IpAddr::V4(ip).is_loopback(),
            Host::Ipv6(ip) => IpAddr::V6(ip).is_loopback(),
        };
        if url.scheme() != "https" && !(url.scheme() == "http" && local) {
            return Err(Error::InvalidEndpoint);
        }
        if self.anonymous && (!local || self.protocol != Protocol::Openai) {
            return Err(Error::LocalOnly);
        }
        let path = url.path().trim_end_matches('/');
        let models = if path.ends_with("/models") { path.to_owned() }
            else if (self.protocol == Protocol::Anthropic && !path.ends_with("/v1"))
                || (path.is_empty() && url.host_str() == Some("api.openai.com")) { format!("{path}/v1/models") }
            else { format!("{path}/models") };
        url.set_path(&models);
        Ok(url)
    }

    fn account(&self, target: &Url) -> String {
        let protocol = match self.protocol { Protocol::Openai => "openai", Protocol::Anthropic => "anthropic" };
        let scoped = format!("{}\0{protocol}\0{target}", self.tool.id());
        format!("api-v1-{}", hex::encode(Sha256::digest(scoped.as_bytes())))
    }
}

fn api_key(input: &str) -> bool {
    // Known browser and OAuth representations are rejected. The source of opaque
    // tokens is established by the user's API-key declaration.
    let prefix = input.as_bytes().iter().copied().take(24).map(|b| b.to_ascii_lowercase()).collect::<Vec<_>>();
    input.len() >= 8 && input.len() <= 1024
        && input.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.~+/=".contains(&b))
        && ![&b"sk-ant-oat"[..], b"sk-ant-sid", b"eyj", b"sess-", b"session-", b"oauth", b"bearer", b"cookie", b"ya29.", b"v1."].iter().any(|p| prefix.starts_with(p))
}

impl Engine {
    pub fn has_key(&self, input: &ProbeInput) -> Result<bool> {
        let target = input.target()?;
        Ok(!input.anonymous && self.vault.get(&input.account(&target))?.is_some())
    }

    pub fn set_key(&self, input: &ProbeInput, secret: Zeroizing<String>) -> Result<()> {
        if input.anonymous || !api_key(&secret) { return Err(Error::InvalidSecret); }
        self.vault.set(&input.account(&input.target()?), &secret)
    }

    pub fn remove_key(&self, input: &ProbeInput) -> Result<()> {
        if input.anonymous { return Err(Error::InvalidSecret); }
        self.vault.remove(&input.account(&input.target()?))
    }

    pub fn probe(&self, input: &ProbeInput) -> Result<ProbeResult> {
        let target = input.target()?;
        let secret = if input.anonymous {
            None
        } else {
            let secret = self.vault.get(&input.account(&target))?.ok_or(Error::MissingSecret)?;
            if !api_key(&secret) { return Err(Error::InvalidSecret); }
            Some(secret)
        };
        let mut builder = Client::builder().no_proxy().redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(12));
        if target.host_str() == Some("localhost") {
            let port = target.port_or_known_default().ok_or(Error::InvalidEndpoint)?;
            let local = [SocketAddr::from((Ipv4Addr::LOCALHOST, port)), SocketAddr::from((Ipv6Addr::LOCALHOST, port))];
            builder = builder.resolve_to_addrs("localhost", &local);
        }
        let client = builder.build().map_err(|_| Error::Io)?;
        let start = Instant::now();
        let result = |status, outcome, models| ProbeResult {
            elapsed_ms: start.elapsed().as_millis() as u64, status, outcome, models,
        };
        let mut request = client.get(target);
        match (input.protocol, secret.as_ref()) {
            (Protocol::Openai, Some(secret)) => {
                let bearer = Zeroizing::new(format!("Bearer {}", secret.as_str()));
                let mut header = HeaderValue::from_str(&bearer).map_err(|_| Error::InvalidSecret)?;
                header.set_sensitive(true);
                request = request.header(AUTHORIZATION, header);
            }
            (Protocol::Anthropic, Some(secret)) => {
                let mut header = HeaderValue::from_str(secret.as_str()).map_err(|_| Error::InvalidSecret)?;
                header.set_sensitive(true);
                request = request.header("x-api-key", header).header("anthropic-version", "2023-06-01");
            }
            (_, None) => {}
        }
        let response = match request.send() {
            Ok(response) => response,
            Err(error) if error.is_timeout() => return Ok(result(None, "timeout", None)),
            Err(_) => return Ok(result(None, "network_error", None)),
        };
        let status = response.status().as_u16();
        if !response.status().is_success() {
            let outcome = match status {
                300..=399 => "redirect",
                401 | 403 => "rejected",
                404 | 405 => "unsupported",
                429 => "rate_limited",
                _ => "http_error",
            };
            return Ok(result(Some(status), outcome, None));
        }
        let mut body = Zeroizing::new(Vec::new());
        if response.take(1_048_577).read_to_end(&mut body).is_err() || body.len() > 1_048_576 {
            return Ok(result(Some(status), "unexpected_response", None));
        }
        let models = serde_json::from_slice::<serde_json::Value>(&body).ok().and_then(|value| {
            let data = value.get("data")?.as_array()?;
            data.iter().all(|entry| entry.get("id").and_then(|id| id.as_str()).is_some()).then_some(data.len())
        });
        Ok(result(Some(status), if models.is_some() { "connected" } else { "unexpected_response" }, models))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_and_limits_targets() {
        let mut input = ProbeInput { tool: Tool::Claude, protocol: Protocol::Anthropic, endpoint: "https://api.anthropic.com/".into(), anonymous: false };
        assert_eq!(input.target().unwrap().as_str(), "https://api.anthropic.com/v1/models");
        for url in ["http://example.com/v1", "https://u:p@foo.test/v1", "https://foo.test/v1?token=x", "file:///tmp/test"] {
            input.endpoint = url.into();
            assert_eq!(input.target(), Err(Error::InvalidEndpoint));
        }
        input.endpoint = "http://[::1]:9000/v1/".into();
        assert_eq!(input.target().unwrap().as_str(), "http://[::1]:9000/v1/models");
        input.endpoint = "https://gateway.example/team/".into();
        assert_eq!(input.target().unwrap().as_str(), "https://gateway.example/team/v1/models");
    }
    #[test]
    fn rejects_browser_and_oauth_tokens() {
        for sample in ["sk-ant-oat01-fake", "sk-ant-sid01-fake", "eyJdummy.token", "Bearer abc", "session-fake", "name=value; x=1"] {
            assert!(!api_key(sample));
        }
        assert!(api_key("sk-ant-api03-test"));
        assert!(api_key("sk-proj-example"));
    }

    #[test]
    fn anonymous_mode_requires_an_openai_compatible_loopback_target() {
        let mut input = ProbeInput { tool: Tool::Opencode, protocol: Protocol::Openai, endpoint: "https://gateway.example/v1".into(), anonymous: true };
        assert_eq!(input.target(), Err(Error::LocalOnly));
        input.endpoint = "http://localhost:1234/v1".into();
        assert_eq!(input.target().unwrap().as_str(), "http://localhost:1234/v1/models");
        input.protocol = Protocol::Anthropic;
        assert_eq!(input.target(), Err(Error::LocalOnly));
    }
}
