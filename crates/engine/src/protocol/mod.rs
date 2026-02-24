/// 支持的 URL scheme。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

impl TryFrom<&str> for Scheme {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "http" => Ok(Scheme::Http),
            "https" => Ok(Scheme::Https),
            other => Err(format!("不支持的 URL scheme: {other}")),
        }
    }
}

/// HTTP 版本策略（用于协议扩展与行为控制）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpVersionPolicy {
    Http1Only,
    Http1OrHttp2,
}

impl HttpVersionPolicy {
    pub fn allows_http2(self) -> bool {
        matches!(self, Self::Http1OrHttp2)
    }
}
