# 架构：pwc-provider

## 1. 模块

```text
pwc-provider/
├── Cargo.toml
├── src/
│   ├── lib.rs          模块声明
│   ├── main.rs         演示程序：参数解析、审批模式、启动
│   ├── error.rs        ErrorCode（36 个）与 PwcError
│   ├── types.rs        语义类型（解析时校验）、资源模型、手续费公式
│   ├── methods.rs      方法表与各方法参数结构
│   ├── backend.rs      WalletBackend trait 及其数据类型
│   ├── memory.rs       MemoryBackend：模拟钱包与模拟链
│   ├── balance.rs      余额计算纯函数（Core §4.3）
│   ├── psbt.rs         PSBT 原始键值解析、combine、finalize、签名前审查、vsize 估算
│   ├── message.rs      BIP-322 simple 与 legacy 消息签名和验证
│   ├── prompt.rs       审批请求类型
│   ├── provider/
│   │   ├── mod.rs      Provider、Config、状态、检查管线、保存与加载
│   │   ├── grants.rs   授权的覆盖判断与收紧
│   │   ├── operations.rs Operation 状态机、可见性
│   │   └── handlers.rs 各方法第 10 步的处理
│   └── local.rs        LOCAL 传输：端点、对端检查、帧、JSON-RPC、握手、事件写出
└── tests/              集成测试，经 Unix socket 访问
```

## 2. 分层

```text
宿主钱包 / 演示程序
   │  Prompt 通道（审批）     save_state / with_state     notify(BackendEvent)
   ▼
local.rs ──帧与握手──► Provider（检查管线、授权、Operation、幂等、预留、事件）
                          │
                          ▼
                    WalletBackend trait ◄── MemoryBackend
```

- **local.rs** 只处理传输：端点、对端用户、帧、JSON-RPC 信封、`local.*` 方法、超时。认证完成后，把 Core 方法交给 `Provider::handle`。
- **Provider** 与传输无关，负责 Core 语义：检查顺序、授权、Operation 状态机、幂等、预留、分页游标、事件扇出、审批流程。
- **WalletBackend** 负责钱包数据与密钥：钱包和账户清单、余额、地址、UTXO、交易记录、输出归属、选币建 PSBT、签名、广播、消息签名。

## 3. 检查管线

`Provider::handle(session, method, params)` 严格按 Core §8 的顺序执行：

| 步骤 | 内容 | 失败时 |
|---|---|---|
| 1 | 已认证（由 local.rs 保证） | `UNAUTHENTICATED` |
| 2 | 请求配额（不实现） | — |
| 3 | 方法在方法表中 | `UNSUPPORTED_METHOD` |
| 4 | 参数语法；大小限制 | `INVALID_ARGUMENT`；`PAYLOAD_TOO_LARGE` |
| 5 | 幂等记录：精确重试直接返回原响应 | `IDEMPOTENCY_CONFLICT` |
| 6 | 有效授权持有所需 scope | `PERMISSION_REVOKED` / `PERMISSION_REQUIRED` |
| 7 | 钱包或账户存在且被授权覆盖 | `WALLET_NOT_FOUND` / `ACCOUNT_NOT_FOUND`，内容固定 |
| 8 | 钱包状态 | `WALLET_LOCKED` / `STATE_UNAVAILABLE` |
| 9 | 未结束的 Operation 数量；审批提示频率 | `RATE_LIMITED` |
| 10 | 方法自身检查（含授权约束） | 各方法的错误码 |

成功时写入幂等记录。直接返回错误的请求不写记录。

## 4. 审批

`Provider::new` 返回 `(Arc<Provider>, PromptReceiver)`。需要用户决定时，Provider 通过通道发送 `Prompt`，每个 `Prompt` 带一个 oneshot 回复端：

| Prompt | 回复 |
|---|---|
| `Pairing` | `bool` |
| `Permissions` | `Option<GrantChoice>`，`None` 为拒绝 |
| `SignPsbt` | `bool` |
| `SignMessage` | `bool` |

- 宿主丢弃回复端视为拒绝。
- Provider 丢弃接收端即撤回提示（连接关闭、取消、过期、授权被撤销）。宿主可用 `reply.is_closed()` 感知。
- 对用户选择的授权，Provider 再做一次收紧：scope 取与请求的交集，过期时间和约束不得比请求宽松，资源不得超出请求（请求列出了资源时）。

每个等待审批的 Operation 对应一个任务，等待三者之一：用户回复、到期、取消信号。用户批准后重新检查授权和钱包状态，再执行签名。

## 5. 状态与并发

- Provider 的全部可变状态放在一个 `Mutex<State>` 中，每个请求在锁内完成。请求因此串行执行，竞争同一 UTXO 的请求自然被序列化（Core §9.3）。
- 后端调用在持锁时进行。后端不回调 Provider，所以不会死锁。
- 每个连接有一个有界输出通道和一个写任务。响应用 `send().await`；事件用 `try_send`，通道满时关闭该连接（LOCAL §8）。
- 连接关闭时通过 `watch` 通道通知该连接上的等待任务（例如配对提示）。

## 6. 状态快照

`State` 中可持久化的部分可序列化为 JSON 快照：

```text
Snapshot { version: 1, cursor_key, clients, grants, operations, idempotency, reservations }
```

不进入快照的：活动会话、限流计数、等待任务。时间一律用 UTC 时间戳，加载后照常比较。

## 7. 依赖

| crate | 用途 |
|---|---|
| tokio | Unix socket、定时器、通道、信号 |
| serde、serde_json | 线格式与快照 |
| bitcoin 0.32 | 地址、交易、PSBT、sighash、secp256k1、HMAC/SHA-256、legacy 消息签名 |
| miniscript 13 | 仅用于 `psbt.finalize` |
| base64 | 规范的带填充 base64 |
| getrandom | 密钥、nonce、标识符的随机源 |
| time | RFC 3339 时间 |
| libc | `geteuid` |
| jsonschema、tempfile（仅测试） | 用 strict schema 校验输出；临时端点目录 |
