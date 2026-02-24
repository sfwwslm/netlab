# crates/engine/README.md

## netlab-engine

`netlab-engine` 是 **netlab 项目的核心引擎**，负责所有 **网络行为构造、调度、执行与指标采集**。

> **这是整个项目中唯一“不可替代”的部分。** GUI、CLI、甚至 Tauri 本身，都只是它的不同外壳。

---

## 1. 设计目标

`netlab-engine` 的目标不是“发请求”，而是：

> **精确地构造、驱动、观测和验证网络行为**

包括但不限于：

* HTTP / TCP / TLS 行为实验
* 连接模型（短连接 / Keep-Alive / HTTP2）
* 并发与调度策略
* 网络异常与边界行为复现
* 高精度指标采集
* 网络调试与代理转发能力（HTTP/HTTPS/SOCKS5）

---

## 2. 核心设计原则

### 2.1 引擎必须是“纯核心”

**明确禁止以下内容进入 engine：**

* ❌ Tauri / GUI 相关代码
* ❌ 前端事件名、UI 状态
* ❌ WebView / JS 交互
* ❌ 平均值导向的“玩具指标”

engine **只关心网络与时间**。

---

### 2.2 可复用、可组合、可替换

engine 必须能够：

* 被 CLI 直接调用
* 被 GUI（Tauri）调用
* 在 headless / CI 环境中运行
* 独立做 benchmark / 实验

---

## 3. 技术选型

| 项目 | 说明 |
| ---- | ------------- |
| 语言 | Rust |
| 异步 | tokio |
| 网络 | hyper |
| 架构 | Library crate |
| 并发模型 | engine 内部完全控制 |

---

## 4. 模块结构

```text
engine/
├── src/
│   ├── lib.rs
│   ├── conn/          # 连接与传输模型
│   ├── scenario/      # 场景定义（压测 / 调试 / 实验）
│   ├── net_debug/     # 网络调试核心
│   ├── proxy/         # HTTP/HTTPS 代理
│   ├── socks5/        # SOCKS5 代理（TCP + UDP ASSOCIATE）
│   ├── scheduler/     # 调度器（并发 / 速率 / 队列）
│   ├── metrics/       # 指标采集与统计
│   └── protocol/     # 协议级扩展（HTTP / TLS / DNS 等）
```

---

## 5. 核心概念

### 5.1 Scenario（场景）

Scenario 定义的是：

* 要做 **什么网络行为**
* 以 **什么节奏**
* 持续 **多久**

而不是 UI 交互。

---

### 5.2 Scheduler（调度器）

Scheduler 负责：

* 并发控制
* 速率限制
* 请求队列
* 生命周期管理

**这是 engine 的“心脏”**。

---

### 5.3 Metrics（指标）

engine 中的指标原则：

* 使用 `Instant`
* 关注分布（p50 / p90 / p99）
* 区分：

  * 连接耗时
  * 首字节时间
  * 完整响应时间

---

## 6. 使用方式（说明）

当前阶段 API 仍在快速演进，示例代码容易过时。建议先关注：

* `engine` 的职责边界与核心概念
* `scenario`、`scheduler`、`metrics` 的组织方式
* CLI/Tauri 层对 `engine` 的调用路径

---

## 6.1 近期已落地能力（摘要）

* 网络调试核心已下沉到 `net_debug`
* 代理能力已落地：
  * HTTP/HTTPS 代理（CONNECT 隧道）
  * SOCKS5 代理（TCP + UDP ASSOCIATE）

---

## 7. 稳定性与演进策略

* 早期阶段允许为提升清晰性进行 API 重构
* 进入稳定期后，再强调兼容性与语义稳定
* 任何变更都需保证核心行为可解释、可验证
* **engine 的代码质量优先级高于 UI**

---

## 8. 当前阶段说明

> 当前阶段，**engine 是 netlab 唯一的开发重点**。

* GUI、CLI 均视为外壳
* 所有新功能优先在 engine 中实现
* engine 必须在没有任何 UI 的情况下“站得住”

---

## 9. 你在这里写的代码，应该具备的气质

* 可解释
* 可验证
* 可复现
* 不讨巧

---
