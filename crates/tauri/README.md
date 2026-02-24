# Tauri 后端（桌面包装层）

本 `crate` 是 `NetLab` 桌面端的 `Tauri` 后端，职责：

- 嵌入前端的 Vite 构建产物。
- 暴露与核心引擎 `crates/engine` 交互的 `invoke` 命令。
- 转发引擎事件（负载测试 / 网络调试 / 代理状态）。

> 架构红线：核心调度/指标/协议逻辑必须留在 crates/engine，这里仅做桥接与包装。

## 开发注意事项

- 保持 `engine` 解耦：勿在此层编写核心逻辑，仅调用 `crates/engine`。
- 新增 `invoke` 时：
  - 校验输入，返回可读错误给前端。
  - 避免阻塞主线程，耗时操作放后台或异步。
  - 仅暴露最小必要能力，并与前端接口对齐。
- 修改配置（tauri.conf.json / Tauri.dev.toml）需兼顾多平台与签名/沙箱要求。

## 当前已对接的能力（摘要）

- 负载测试（Load Test）
- 网络调试（Network Debug）
- HTTP/HTTPS 代理
- SOCKS5 代理（含 UDP ASSOCIATE）
- 日志中心窗口（跨功能日志订阅）
