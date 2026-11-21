# 需求入口：PWC LOCAL Provider（Rust 实现）

本文件是需求的唯一入口。需求变化时先改本文件，再同步 `docs/requirements/product-spec.md`、`docs/architecture.md`、`docs/api-contract.md` 和 `docs/tasks/`。

## 1. 目标

按 `specs/` 中的规范实现 PWC 的服务端，也就是 Provider（钱包端）：

- 实现 Core v0.2.1（`specs/PWC-Core-v0.2.1-EN.md`）定义的全部 24 个方法；
- 通过 LOCAL 传输 v0.1.0（`specs/PWC-Transport-Local-v0.1.0-EN.md`）对外提供服务：Unix socket，JSON-RPC 2.0，配对，HMAC-SHA256 双向认证，事件推送。

规范没有 HTTP 接口。LOCAL 传输禁止任何 TCP 或 UDP 监听。

## 2. 已确认的决定

| 项 | 决定 |
|---|---|
| 语言 | Rust，单 crate，放在 `pwc-provider/` |
| 方法范围 | Core 全部方法，含 `psbt.create`、`psbt.sign`、`psbt.combine`、`psbt.finalize`、`psbt.releaseReservation`、`transaction.broadcast`、`message.sign`、`message.verify` |
| 钱包后端 | 定义后端 trait，第一版提供内存模拟实现，不接真实节点 |
| 网络 | `mainnet`、`signet`、`regtest`。Purity 的地址格式与 Bitcoin Core 完全相同 |
| 脚本类型 | `p2wpkh`（可建 PSBT、可签 PSBT）、`p2pkh`（只读与 `legacy` 消息签名） |
| 消息签名格式 | `bip322-simple`（P2WPKH）、`legacy`（P2PKH） |
| 持久化 | Provider 自身不写文件。提供保存和加载状态的库接口，由宿主钱包调用并负责存储 |
| 事件推送 | 实现，声明 `event_push` |
| 用户审批 | 通过通道交给宿主界面。演示程序有终端问答与自动批准两种模式 |
| 开发方式 | 测试先行 |

## 3. 不做的范围

- 委托主体（LOCAL §5.8）：`local.openDelegate`、`local.delegated`、`local.closeDelegate` 返回 `UNSUPPORTED_OPERATION`，`local.hello` 中 `delegation` 为 `false`。
- Windows 命名管道、REMOTE、BROWSER、MOBILE 传输。
- `awaiting_unlock` 状态：锁定钱包上的操作直接返回 `WALLET_LOCKED`。
- 请求配额限流（Core §8 检查 2，规范为 MAY）。
- Taproot、多签账户、硬件签名器。
- 演示程序不做持久化。

## 4. 验收

- `cargo test` 全部通过，测试覆盖 `docs/tasks/README.md` 中列出的 C-xx 和 L-xx 测试编号。
- 演示程序以自动批准模式运行时，`pwc-testkit/local_runner.py --pair --allow-destructive` 没有失败项。
- 进程没有任何 TCP 或 UDP 监听。
- 仓库原有的 `run_vectors.py` 和 `check_schema.py` 结果不变。
