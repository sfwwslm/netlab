use std::time::Duration;

/// 连接复用策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionMode {
    /// 复用 keep-alive 连接（默认，适合吞吐/稳态压测）。
    KeepAlive,
    /// 每个请求使用全新的连接（不复用），适合观察握手/accept/TIME_WAIT 压力。
    NewConnection,
}

impl ConnectionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepAlive => "keepAlive",
            Self::NewConnection => "newConnection",
        }
    }

    pub fn pool_config(self) -> ConnectionPoolConfig {
        match self {
            Self::KeepAlive => ConnectionPoolConfig {
                max_idle_per_host: 512,
                idle_timeout: Duration::from_secs(30),
                close_header: false,
            },
            Self::NewConnection => ConnectionPoolConfig {
                max_idle_per_host: 0,
                idle_timeout: Duration::from_secs(0),
                close_header: true,
            },
        }
    }
}

/// 连接池/复用相关参数（与具体客户端实现解耦）。
#[derive(Clone, Copy, Debug)]
pub struct ConnectionPoolConfig {
    pub max_idle_per_host: usize,
    pub idle_timeout: Duration,
    pub close_header: bool,
}
