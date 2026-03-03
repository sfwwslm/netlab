# 负载测试（Load Test）功能说明（V1）

本文档描述当前 netlab 桌面端的“负载测试”功能的**现有逻辑**与**架构边界**，并给出对标专业压测工具（wrk/k6/vegeta/hey 等）时的不足与后续演进方向。

> 说明：
>
> - 本文档聚焦“现在代码是怎么做的”，不作为长期架构宪法（长期约束见 `ARCHITECTURE_V1.md`）。
> - 当前阶段 GUI/Tauri 仍是壳层，核心执行逻辑放在 `engine` 内，遵循 `CONTRIBUTING.md` 的边界要求。

---

## 1. 功能定位

负载测试是一个用于快速施加 HTTP 请求负载、实时观察关键指标（成功率、延迟、RPS 等）的实验能力。

当前实现侧重：

- 在本机/局域网目标上快速跑出“可复现”的负载曲线
- 实时 UI 观测 + 本地历史记录留存
- 通过模式开关降低压测端自扰动（便于“冲吞吐”）

不追求：

- 取代专业压测工具的全部能力（分布式、脚本化、复杂场景编排等）

---

## 2. 架构分层与职责边界

### 2.1 分层

```text
frontend (React UI)
    ↓ invoke / event
tauri (adapter + history store)
    ↓ engine API
engine (core load test runner)
```

### 2.2 边界原则

- `engine`：承载负载测试的并发执行、网络请求、计时与指标采集；不包含任何 UI 事件语义。
- `tauri`：仅做参数转换、事件转发、历史记录持久化（SQLite）；不实现调度与指标计算。
- `frontend`：展示、配置、可视化；不实现核心逻辑。

---

## 3. 当前实现：端到端流程

### 3.1 启动/停止

1. 前端调用 `invoke("start_load_test", { config })`
2. Tauri 将前端配置转换为 `engine::scenario::load_test::LoadTestConfig`
3. `engine` 启动负载测试任务并产出事件流
4. Tauri 订阅事件流并转发到前端：
   - `loadtest:metrics`：周期性指标快照（以及最终 done=true 的快照）
   - `loadtest:log`：错误日志（请求失败/超时等）
5. 前端停止时调用 `invoke("stop_load_test")`
6. Tauri 设置 stop flag 并 abort 后台任务

对应代码入口：

- Tauri invoke：`src-tauri/src/invokes/load_test.rs`
- Engine runner：`crates/engine/src/scenario/load_test.rs`

### 3.2 实时指标上报

`engine` 内部会以固定间隔（默认 500ms）发送一次指标快照事件。

指标快照包含（UI 使用）：

- `total_requests` / `success` / `failures`
- `avg_latency_ms` / `p95_ms` / `p99_ms`
- `rps` / `throughput_bps` / `total_bytes` / `avg_bytes_per_request`
- `status_codes`：状态码分布（包含 `status_no_response` / `status_other`）
- `completion_buckets`：P50/P90/P95/P99/P100
- `done`：是否完成

### 3.3 历史记录（SQLite）

当收到 `done=true` 的最终快照时，Tauri 会写入 SQLite：

- 表：`load_test_history`
- 记录内容：基本配置（url/method/concurrency/duration/timeout/headers）+ 汇总指标（success_rate/avg_latency/rps/p95/p99）

相关代码：`src-tauri/src/invokes/load_test.rs` 中的 `save_history/query_history/...`

---

## 4. 配置项（V1）

### 4.1 基础配置（前端输入）

- `url`：目标地址（http/https）
- `method`：GET/POST
- `concurrency`：并发 worker 数（每个 worker 循环发送请求）
- `durationSecs`：持续时间
- `timeoutMs`：单次请求超时
- `headers`：可选请求头
- `payload`：可选请求体（POST）
- `allowInsecureCerts`：允许不安全证书（仅 https 生效；用于自签证书/内网目标；开启后会跳过证书校验）

### 4.2 响应处理模式（`responseMode`）

用于降低压测端自身开销（尤其是“冲吞吐”场景）。

- `countBytes`（默认）：读取响应体并统计字节数，用于 `throughput_bps` 计算。
- `discardBody`：读取响应体但丢弃内容（仍会 drain body，以保证连接可复用），字节统计为 0。

注意：

- 对于“超小响应体”（例如 `Hello World`）目标，`discardBody` 与 `countBytes` 的差异可能不明显，因为读取 body 的成本非常低，瓶颈常在协议/调度/内核栈。

### 4.3 指标计算模式（`metricsMode`）

用于降低 percentile 计算等统计成本，进一步压榨吞吐。

- `full`（默认）：采集延迟样本并计算分位（P50/P90/P95/P99/P100），UI 图表完整。
- `minimal`：不采集延迟样本，不计算分位（p95/p99/buckets 置空或为 0），仅保留计数、均值、RPS。

---

## 5. 计时与统计口径（V1）

### 5.1 延迟定义

当前“单次请求延迟”口径为：

- 从发送请求开始，到响应体读取完成（drain 完成）的总耗时（wall-clock）

这意味着它更接近“完整响应耗时”，而不是更细粒度的：

- DNS 时间 / 连接建立时间
- TTFB（首字节时间）
- TLS handshake

### 5.2 统计策略（性能优先）

为了降低压测端自扰动，`engine` 的统计做了多处性能取舍：

- 热路径计数使用原子累加，降低锁竞争
- 延迟样本采用 worker 本地缓冲，批量 flush 到共享 reservoir
- 分位计算在锁外排序，避免影响请求热路径
- `metricsMode=minimal` 直接跳过分位相关计算

---

## 6. 与专业工具对比：当前不足

与 wrk/k6/vegeta/hey/bombardier 等专业压测工具相比，当前 V1 实现主要不足包括：

### 6.1 负载模型与调度能力不足

- 已支持固定 RPS（全局/每 worker），仍缺少阶段化调度
- 缺少 ramp-up/ramp-down、阶段化脚本、场景组合
- 已支持短连接/keep-alive 模式，仍缺少连接池上限等细粒度策略

### 6.2 指标体系不够专业

- 目前以“总耗时”作为单指标，缺少 TTFB/连接耗时/握手耗时等拆分
- 分位计算策略偏简单（reservoir + sort），与 HDRHistogram/t-digest 等成熟方案相比：
  - 精度/稳定性不足（尤其高样本量与极端长尾）
  - 内存/CPU 成本可控性较弱
- 缺少更完整的错误分类（连接错误/读超时/写超时/HTTP 状态分布等）

### 6.3 协议覆盖与兼容性不足

- HTTP/2/HTTP/3 的系统化支持、针对性的连接/多路复用控制仍不足
- TLS 细粒度配置（SNI、cipher suites、证书校验策略）不完善

### 6.4 可观测性与可重复性不足

- 缺少“测试元数据 + 环境信息”记录（CPU/OS/进程、版本、目标信息）
- 缺少稳定的结果导出格式（如 wrk/k6 的报告、Prometheus/OpenTelemetry）
- 已提供 headless/CI 入口（netlab-cli，TUI 形式）

---

## 7. 后续演进方向（建议）

以下方向建议优先落在 `engine`（保持壳层轻量）：

### 7.1 Scheduler 已引入（基础版）

已将负载测试从“并发循环”演进为“场景 + 调度器”，目前为基础版：

- 支持固定 RPS、突发（burst）、阶段（stage）
- 支持 warmup、steady、cooldown
- 支持多种并发/连接策略（短连接/keep-alive/连接池限制）

### 7.2 指标系统升级

- 使用 HDRHistogram 或更成熟的分位统计结构（降低计算成本并提升稳定性）
- 指标拆分：TTFB、总耗时、连接耗时、错误分类、状态码分布
- 输出标准化：JSON/CSV + 可选 Prometheus/OpenTelemetry

### 7.3 场景化与可复现

- 将“负载测试”视为 `scenario` 的一个实现，与其他网络实验统一
- 支持场景定义版本化/可导入导出

### 7.4 壳层增强但不侵入核心

- UI 提供更明确的模式提示（吞吐优先 vs 指标完整）
- 历史记录可保存更多元数据（配置、版本、运行环境摘要）

---

## 8. 使用建议（现阶段）

### 8.1 做公平对比（延迟分布）

在比较不同实现/不同版本时，建议保持“同等负载”：

- 固定 RPS（未来建议新增），或
- 调整并发使 RPS 接近一致

否则在更高负载下 P95/P99 上升是正常现象，不能直接归因于实现退化。

### 8.2 冲吞吐（找上限）

- `responseMode=discardBody`
- `metricsMode=minimal`
- 逐步提高并发，观察 RPS 增长是否进入平台期（平台期通常意味着目标或系统瓶颈）

### 8.3 选择连接模式

- `connectionMode=keepAlive`（默认）：复用连接池，适合吞吐/稳态压测。
- `connectionMode=newConnection`：每个请求携带 `Connection: close` 并关闭连接池，适合观察握手/accept/TIME_WAIT 压力或服务端连接上限。

---
