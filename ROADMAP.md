# Roadmap

以下为 netlab "专业压测工具" 方向的缺失能力规划。按六大类组织，每条包含优先级、模块位置与依赖，便于拆分为 issue。

## 调度与负载模型

- P0 阶段化计划（ramp-up/steady/ramp-down）
  - 模块: `crates/engine/src/scheduler/`
  - 配置: `SchedulePlan { stages: Vec<Stage { duration, target_rps|concurrency }> }`
  - 边界: UI 仅配置/展示，engine 执行调度
  - 依赖: 现有 `RpsMode`/`ScheduleConfig` 扩展
- P0 Open/Closed Model 切换
  - 模块: `crates/engine/src/scheduler/`
  - 配置: `LoadModel::{OpenArrival(rate), ClosedConcurrency(n)}`
  - 依赖: 阶段化计划
- P1 到达率分布（均匀/泊松）
  - 模块: `crates/engine/src/scheduler/`
  - 配置: `ArrivalDistribution::{Uniform, Poisson(lambda)}`
  - 依赖: Open Model 实现

## 协议与连接层

- P0 HTTP/2 并发 stream 与连接上限
  - 模块: `crates/engine/src/protocol/`, `crates/engine/src/conn/`
  - 配置: `Http2Config { max_concurrent_streams, max_connections }`
  - 边界: Tauri/Frontend 只透传配置
  - 依赖: `protocol::HttpVersionPolicy` 扩展
- P1 TLS 版本/密码套件策略
  - 模块: `crates/engine/src/protocol/`
  - 配置: `TlsConfig { min_version, max_version, cipher_suites }`
  - 依赖: 现有 rustls 构建流程
- P1 DNS/连接池策略
  - 模块: `crates/engine/src/conn/`
  - 配置: `ConnectionPoolConfig { max_idle, idle_timeout, resolve_ttl }`
  - 依赖: `ConnectionMode` 已有字段

## 请求脚本与数据驱动

- P0 多步骤场景（流程编排）
  - 模块: `crates/engine/src/scenario/` 新增 `ScenarioPlan`
  - 配置: `ScenarioStep { request, assert, next }`
  - 边界: UI 仅编辑/导出，engine 运行
  - 依赖: 统一请求模型
- P1 变量/参数化（CSV/随机/分布）
  - 模块: `crates/engine/src/scenario/`, `crates/engine/src/protocol/`
  - 配置: `DataSource::{Csv, Random, Distribution}` + `TemplateVar`
  - 依赖: 多步骤场景
- P1 响应断言（状态码/体/延迟阈值）
  - 模块: `crates/engine/src/scenario/`
  - 配置: `Assertion::{Status, BodyContains, LatencyLT}`
  - 依赖: 多步骤场景

## 指标与可观测性

- P0 更精确分布（HDR/t-digest）
  - 模块: `crates/engine/src/metrics/`
  - 配置: `DistributionMode::{Reservoir, HDR, TDigest}`
  - 依赖: 现有 snapshot 输出结构扩展
- P1 指标按路径/状态码分组
  - 模块: `crates/engine/src/metrics/`
  - 配置: `MetricTag { path, status, error_type }`
  - 依赖: 请求模型统一、输出协议
- P2 资源指标采集（CPU/内存/连接数）
  - 模块: `crates/engine/src/metrics/`（或新 `crates/engine/src/telemetry/`）
  - 配置: `SystemMetricsConfig { enabled, interval }`
  - 依赖: metrics 框架扩展

## 分布式与可扩展

- P1 多节点协同（controller/agent）
  - 模块: `crates/engine` + 新 `crates/agent`
  - 配置: `ClusterConfig { agents, plan }`
  - 边界: 壳层只负责启动/配置分发
  - 依赖: 序列化计划/结果协议
- P2 结果汇聚与时间对齐
  - 模块: `crates/engine/src/metrics/`
  - 配置: `MergePolicy { window_ms, skew_ms }`
  - 依赖: 多节点协同
- P2 分布式配置分发与复现
  - 模块: `agent/controller` 协议
  - 配置: `PlanId`, `ArtifactHash`
  - 依赖: 多节点协同

## 工程化

- P0 完整 CLI（脚本化、JSON config）
  - 模块: `crates/cli/`
  - 配置: `cli::Config` ↔ `engine::LoadTestConfig`
  - 边界: CLI 仅解析/调用 engine
  - 依赖: 核心计划/协议配置稳定
- P1 结果导出（CSV/Prometheus）
  - 模块: `crates/engine/src/metrics/` + `crates/cli/`
  - 配置: `ExportFormat::{CSV, Prometheus}`
  - 依赖: 指标结构扩展
- P2 历史对比/回归报告
  - 模块: `crates/tauri` + `crates/engine/src/metrics/`（统计逻辑在 engine）
  - 配置: `BaselineCompareConfig { window, thresholds }`
  - 依赖: 可复现计划、指标分布
