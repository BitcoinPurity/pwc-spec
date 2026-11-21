# 产品规格：pwc-provider

与 `input/requirements.md` 保持一致。规范原文以 `specs/` 为准，本文只写本实现的取舍。

## 1. 产品形态

| 交付物 | 说明 |
|---|---|
| 库 `pwc_provider` | 供宿主钱包嵌入：协议层、LOCAL 传输、后端 trait、内存模拟后端、状态保存与加载 |
| 演示程序 `pwc-provider` | 用内存模拟后端启动一个 LOCAL Provider，供客户端开发与一致性测试使用 |

## 2. 功能清单

### 2.1 LOCAL 传输

- 端点目录按 LOCAL §3.2 选择，也可用 `--dir` 指定。目录必须为当前用户所有，且对组和其他用户无任何权限，否则拒绝启动。socket 文件权限为 0600。
- socket 路径超过系统上限时拒绝启动，不换位置。
- 已有 socket 文件时先尝试连接：连接被拒说明是残留文件，替换它；连接成功说明已有实例在运行，拒绝启动。
- 每个连接检查对端进程的用户，不是同一用户就直接关闭。
- 帧格式：每行一个 JSON-RPC 2.0 对象，`max_frame_bytes` 为 1048576。
- 握手：`local.hello` → `local.pair` 或 `local.authenticate`。时限为 10 秒和 60 秒。
- 配对码 6 位数字。配对提示 300 秒无应答返回 `OPERATION_EXPIRED`；连接关闭时撤回提示，不创建凭据。
- 认证失败按对端进程限流，并延迟应答。
- `local.unpair`：删除凭据，撤销该客户端全部授权，关闭它的全部连接。
- 事件推送：持有 `events:read` 的客户端在每个已认证连接上收到 `pwc.event` 通知。客户端读得太慢导致缓冲超限时关闭该连接。

### 2.2 Core 方法

全部 24 个方法，见 `docs/api-contract.md` 的方法表。请求按 Core §8 的十步检查顺序处理。

### 2.3 内存模拟后端

- 三个网络各有一个模拟链，链之间互相隔离。
- 每个网络两个钱包：
  - 软件钱包，含一个 P2WPKH 账户和一个 P2PKH 账户，都可签名；
  - 观察钱包，含一个 P2WPKH 账户，不可签名。
- 各账户预置若干已确认 UTXO。
- 模拟链支持：广播（校验输入存在、未被花费、签名有效）、出块确认、重组回退、交易被替换、离线开关、用户在钱包内自行花费。
- 钱包状态可切换为 `ready`、`locked`、`unavailable`。

### 2.4 用户审批

需要用户决定的四类事项通过 `Prompt` 通道交给宿主：配对、权限请求、PSBT 签名、消息签名。宿主丢弃回复端视为拒绝；Provider 丢弃接收端表示撤回提示。

演示程序：

- `--approve ask`（默认）：在终端打印提示内容，读取 y/n。
- `--approve auto`：延迟 `--approve-delay-ms`（默认 300 毫秒）后自动批准，启动时在标准错误打印警告。权限请求按请求的范围授予；未指定网络时选 `regtest`，未指定钱包时授予该网络的全部钱包。

### 2.5 状态保存与加载

- `Provider::save_state()` 导出 JSON 快照：已配对客户端（含 `client_secret`）、授权、Operation、幂等记录、预留和游标密钥。
- `Provider::with_state(...)` 从快照恢复。版本不识别或内容损坏时返回错误。
- 快照含密钥。存放位置和保护强度由宿主钱包负责，至少与钱包文件相同（LOCAL §7）。
- 加载时：仍在等待审批的 Operation 改为 `failed`，错误码 `INTERNAL_ERROR`，`retryable: true`；超过保留期的记录丢弃。

## 3. 能力声明

| 字段 | 取值 |
|---|---|
| `profiles` | `pwc-core-read-v1`、`pwc-psbt-signer-v1` |
| `supported_networks` | `mainnet`、`signet`、`regtest` |
| `features` | `coin_control`、`watch_only`、`event_push` |
| `script_types` | `p2wpkh`、`p2pkh` |
| `message_formats` | `bip322-simple`、`legacy` |

`limits` 的具体数值见 `docs/api-contract.md`。

## 4. 持续集成

`.github/workflows/ci.yml` 有两个任务：规范检查（`check_schema.py`、`run_vectors.py`、`check_family.py`）和 Rust Provider（`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`，后者包含对演示程序运行 `local_runner.py`）。Rust 工具链固定为 1.97.0。

## 5. 已知限制

- 跨重启保留状态依赖宿主钱包调用保存与加载接口。演示程序重启后需要重新配对。
- `mainnet` 和 `signet` 下同样是内存模拟数据。
- Core 的 C-xx 测试没有官方 runner，由本工程的集成测试覆盖，不能据此声明通过官方一致性认证。
- 不支持 Taproot、多签、委托。`psbt.combine` 遇到 Taproot 签名字段冲突时直接返回 `PSBT_CONFLICT`，不验签。
