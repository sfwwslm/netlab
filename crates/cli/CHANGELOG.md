# 更新日志

## 2025-12-20

### 新增

- `net-debug` TUI：网络调试收发与日志展示。
- `proxy` TUI：HTTP/HTTPS 代理监控（含 Top 目标）。
- `socks5` TUI：SOCKS5 代理监控（可开启 UDP）。

## 2025-12-19

### 新增

- 基于 `engine::scenario::load_test` 的 CLI 压测入口。
- TUI 实时面板（ratatui + crossterm），展示指标、状态码与日志。
- CLI 参数：并发、时长、超时、headers、body、连接模式、指标模式、RPS 限速/模式、不安全证书。
