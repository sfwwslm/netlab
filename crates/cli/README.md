# crates/cli/README.md

## netlab-cli

`netlab-cli` 是 `netlab` 项目的 **命令行入口**，用于在 **无 GUI 场景** 下运行 `netlab-engine`，`基于 ratatui 的 TUI CLI`。

> **当前阶段：可用（TUI 形式）**

---

## 1. 定位说明（重要）

`netlab-cli` 的存在目的只有一个：

> **为 engine 提供一个最小、可用、可自动化的启动方式**

它 **不是** 项目的重点，也 **不会** 追求与 GUI 等量齐观的功能完整度。

---

## 2. 当前开发策略

* CLI **不会抢占 engine 的设计精力**
* CLI **保持轻量**，以 TUI 方式提供可用入口
* GUI（Tauri）继续作为主要可视化入口

---

## 3. 职责范围

`netlab-cli` 只负责：

* 参数解析
* 启动 engine
* TUI 展示与交互（终端内实时指标）

**明确不负责：**

* 指标计算
* 并发调度
* 网络逻辑
* 场景设计

---

## 4. 技术选型

| 项目 | 说明 |
| ---- | ---- |
| 类型 | Binary crate |
| 依赖 | netlab-engine |
| 参数解析 | clap |
| TUI | ratatui + crossterm |

---

## 5. 使用方式（说明）

当前阶段 CLI 参数仍在调整，建议以 `netlab-cli --help` 为准。

已提供的 TUI 子命令（摘要）：

* `bench`：HTTP 负载测试
* `net-debug`：网络调试
* `proxy`：HTTP/HTTPS 代理监控
* `socks5`：SOCKS5 代理监控（可开启 UDP）

运行后可在终端内查看实时指标，按 q/Esc 退出。

`bench` 常用参数（节选）：

* `--concurrency`：并发数（线程/用户数）
* `--ramp-up`：并发爬升时间（秒）
* `--iterations-per-worker`：每个并发的最大请求次数
* `--total-requests-limit`：全局总请求数上限
* `--duration`：持续时间（秒）

结束条件满足任意一项即停止：持续时间 / 每用户次数 / 总请求数上限。

---

## 6. 为什么现在不重点做 CLI？

原因很明确：

1. engine 尚在快速演进期
2. CLI 过早稳定会反向束缚 engine
3. GUI 更适合早期验证交互与可视化

---

## 7. 开发约束

* CLI 不得引入业务逻辑
* CLI 不得绕过 engine API
* CLI 不得成为“第二套实现”

---

## 8. 当前状态总结

> **netlab-cli 是一个“可用但保持轻量”的模块**

它存在，是为了让架构完整；
它克制，是为了让核心更强。

---

### 最后一句话

> **engine 是 netlab 的灵魂，
> CLI 只是其中一个出口。**

---
