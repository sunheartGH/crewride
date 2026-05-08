# CrewRide 生产级改进规范

## 1. 项目概述

**项目名称**: CrewRide
**项目类型**: AI 接口适配转发代理服务
**核心功能**: 支持 OpenAI、Anthropic、Gemini API 格式之间的无缝转换和转发
**目标**: 从概念验证升级为生产可用的企业级代理服务

## 2. 核心改进目标

### 2.1 必须实现 (P0)
- [ ] 流量统计模块 - 请求计数、Token 用量、分组统计
- [ ] 完善的错误处理 - 结构化错误响应、错误日志
- [ ] API Key 安全处理 - 隐藏敏感信息

### 2.2 生产必备 (P1)
- [ ] 限流和配额机制 - 基于 API Key 的速率限制
- [ ] 健康检查端点 - `/health`, `/ready`
- [ ] 监控指标端点 - Prometheus 格式指标
- [ ] 优雅关闭 - 信号处理、请求完成后再关闭

### 2.3 可靠性增强 (P2)
- [ ] 重试机制 - 指数退避重试
- [ ] 熔断器模式 - 快速失败、自动恢复
- [ ] 请求验证 - 输入参数校验
- [ ] 请求追踪 - Request ID 生成和传递

### 2.4 代码质量 (P3)
- [ ] 单元测试覆盖
- [ ] 配置格式统一
- [ ] 文档完善

## 3. 架构设计

### 3.1 新增模块

```
src/
├── main.rs              # 入口，优雅关闭
├── lib.rs
├── config.rs            # 配置管理（增强）
├── handlers.rs          # 路由分发（增强）
├── handlers/
│   ├── anthropic.rs     # Anthropic 端点
│   ├── openai.rs        # OpenAI 端点
│   ├── gemini.rs        # Gemini 端点
│   └── conversion/      # 响应转换（新增目录）
│       ├── anthropic.rs
│       ├── openai.rs
│       └── gemini.rs
├── stats.rs             # 流量统计模块（新增）
├── middleware/           # 中间件（新增）
│   ├── rate_limit.rs    # 限流中间件
│   ├── request_id.rs    # 请求追踪
│   ├── logging.rs       # 结构化日志
│   └── error_handler.rs # 错误处理
├── health.rs            # 健康检查（新增）
├── metrics.rs           # Prometheus 指标（新增）
├── retry.rs             # 重试机制（新增）
├── circuit_breaker.rs   # 熔断器（新增）
└── error.rs             # 统一错误类型（新增）
```

### 3.2 新增 API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查（存活探针） |
| `/ready` | GET | 就绪检查（依赖服务） |
| `/metrics` | GET | Prometheus 格式指标 |
| `/stats` | GET | 流量统计摘要 |
| `/stats/{provider}` | GET | 按提供商统计 |
| `/stats/{model}` | GET | 按模型统计 |

### 3.3 配置格式增强

```yaml
# config.yaml - 新增字段
host: "127.0.0.1"
port: 8899

# 新增: 统计配置
stats:
  enabled: true
  retention_days: 30
  export_interval_secs: 60

# 新增: 限流配置
rate_limit:
  enabled: true
  default_rpm: 60        # 默认每分钟请求数
  default_tpm: 100000    # 默认每分钟 Token 数
  burst: 10              # 突发容量

# 新增: 重试配置
retry:
  enabled: true
  max_attempts: 3
  initial_delay_ms: 100
  max_delay_ms: 5000
  backoff_multiplier: 2.0

# 新增: 熔断器配置
circuit_breaker:
  enabled: true
  failure_threshold: 5   # 失败阈值触发熔断
  success_threshold: 2    # 成功次数恢复
  timeout_secs: 30        # 熔断超时

# 新增: 超时配置
timeout:
  connect_secs: 10
  read_secs: 60
  write_secs: 60

# 新增: 优雅关闭
shutdown:
  grace_period_secs: 30

providers:
  - key: "openai-official"
    type: "openai"
    api_key: "${OPENAI_API_KEY}"
    api_url: "https://api.openai.com/"
    enabled: true
    # 新增: 提供商级别限流覆盖
    rate_limit:
      rpm: 100
      tpm: 200000

models:
  - model: "gpt-4"
    provider: "openai-official"
    replace:
      api_key: true
      model: "gpt-4o"
    # 新增: 模型级别限流
    rate_limit:
      rpm: 30
      tpm: 50000
```

## 4. 流量统计设计

### 4.1 统计指标

| 指标名 | 类型 | 说明 |
|--------|------|------|
| `requests_total` | Counter | 总请求数 |
| `requests_by_provider` | Counter | 按提供商统计 |
| `requests_by_model` | Counter | 按模型统计 |
| `tokens_total` | Counter | Token 总数 |
| `tokens_input` | Counter | 输入 Token |
| `tokens_output` | Counter | 输出 Token |
| `latency_seconds` | Histogram | 请求延迟分布 |
| `errors_total` | Counter | 错误总数 |
| `errors_by_type` | Counter | 按错误类型统计 |

### 4.2 存储设计

```rust
// 内存存储 + 可选持久化
struct StatsStore {
    requests: AtomicU64,
    tokens_input: AtomicU64,
    tokens_output: AtomicU64,
    by_provider: RwLock<HashMap<String, ProviderStats>>,
    by_model: RwLock<HashMap<String, ModelStats>>,
    time_window: RingBuffer<MinuteStats>,
}

struct ProviderStats {
    requests: AtomicU64,
    tokens_input: AtomicU64,
    tokens_output: AtomicU64,
    errors: AtomicU64,
}

struct ModelStats {
    requests: AtomicU64,
    tokens_input: AtomicU64,
    tokens_output: AtomicU64,
    errors: AtomicU64,
}
```

## 5. 错误处理设计

### 5.1 统一错误类型

```rust
enum ProxyError {
    // 上游服务错误
    UpstreamError { provider: String, message: String },
    Timeout { provider: String },
    CircuitOpen { provider: String },

    // 限流错误
    RateLimitExceeded { limit: u32, window: String },

    // 请求错误
    InvalidRequest { field: String, message: String },

    // 内部错误
    Internal { message: String },
}
```

### 5.2 错误响应格式

```json
{
  "error": {
    "type": "rate_limit_exceeded",
    "message": "请求速率超出限制",
    "details": {
      "limit": 60,
      "window": "per minute",
      "retry_after": 30
    },
    "request_id": "req_abc123"
  }
}
```

## 6. 实现顺序

1. **错误处理基础设施** - 统一错误类型、中间件
2. **结构化日志** - 请求追踪、日志中间件
3. **流量统计** - 核心统计模块
4. **限流机制** - 基于内存的限流
5. **健康检查** - /health, /ready 端点
6. **Prometheus 指标** - /metrics 端点
7. **重试机制** - 带指数退避
8. **熔断器** - 故障检测和恢复
9. **优雅关闭** - 信号处理
10. **单元测试** - 关键模块测试

## 7. 测试策略

- 单元测试: 配置解析、错误处理、统计计算
- 集成测试: 各端点功能测试
- 性能测试: 基准测试

## 8. 提交规范

### 提交信息格式 (英文)
```
<type>(<scope>): <subject>

<body>
```

### Type 类型
- `feat`: 新功能
- `fix`: 错误修复
- `docs`: 文档更新
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建/工具

### 示例提交
```
feat(stats): add traffic statistics module

- Add request counter by provider and model
- Add token usage tracking
- Add Prometheus-compatible metrics endpoint
```

```
fix(security): mask API keys in logs

- Remove API key from debug output
- Add request_id to error responses
```

```
refactor(handler): unify error handling

- Add error middleware
- Return structured error responses
- Add circuit breaker pattern
```
