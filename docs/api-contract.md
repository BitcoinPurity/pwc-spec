# API 契约：pwc-provider

本文写明本实现对外的接口，以及规范留给实现选择的地方取什么值。字段定义、类型和错误码含义以 `specs/PWC-Core-v0.2.1-EN.md`、`specs/PWC-Transport-Local-v0.1.0-EN.md` 和 `pwc-schema/` 为准，这里不再复制。

## 1. 传输

| 项 | 取值 |
|---|---|
| 传输 | LOCAL v0.1.0，协议 `pwc-local/1` |
| `provider_id` | `pwc-provider` |
| 端点 | `<端点目录>/pwc-provider.sock`，目录 0700，socket 0600 |
| `max_frame_bytes` | 1048576 |
| `delegation` | `false` |
| JSON-RPC 错误 | 解析失败 `-32700`；非法请求、批量、重复的未完成 id 为 `-32600`；其余一律 `1000`，`data` 为完整 `PwcError`，`message` 与 `data.message` 相同 |

### 1.1 `local.*` 方法

| 方法 | 允许的连接状态 | 说明 |
|---|---|---|
| `local.hello` | 新连接 | 必须是第一个请求。没有共同协议时返回 `UNSUPPORTED_OPERATION` 并关闭。参数畸形时返回 `INVALID_ARGUMENT`，连接保持 |
| `local.pair` | 已 hello | 每个连接只允许一次配对提示，第二次返回 `RATE_LIMITED`。`delegator: true` 返回 `UNSUPPORTED_OPERATION`，不弹提示 |
| `local.authenticate` | 已 hello | 每个连接只允许一次尝试。失败返回固定内容的 `UNAUTHENTICATED` 并关闭；失败应答延迟 100 毫秒。参数畸形返回 `INVALID_ARGUMENT` 并关闭 |
| `local.unpair` | 已认证 | 返回 `{"unpaired": true}` 后关闭该客户端的全部连接 |
| `local.openDelegate`、`local.delegated`、`local.closeDelegate` | 已认证 | `UNSUPPORTED_OPERATION` |

其他规则：

- 新连接的第一个请求不是 `local.hello`：返回 `UNAUTHENTICATED` 并关闭。
- 已 hello、未认证时调用 Core 方法：返回 `UNAUTHENTICATED`，连接保持。
- 再次调用 `local.hello`、或认证后调用 `local.pair` 与 `local.authenticate`：返回 `UNSUPPORTED_OPERATION`，连接保持。
- 配对进行中调用 `local.authenticate`：返回 `UNSUPPORTED_OPERATION`。
- 认证后调用未知的 `local.*` 方法：返回 `UNSUPPORTED_METHOD`。
- 帧超长：返回 `id` 为 `null` 的 `PAYLOAD_TOO_LARGE`，然后关闭。
- 限流：
  - 配对提示按对端进程计，每 60 秒最多 10 次；
  - 认证失败按对端进程计，每 60 秒最多 10 次；
  - 超限返回 `RATE_LIMITED`；认证超限时同时关闭连接。

## 2. 能力（`pwc.getCapabilities`）

| 字段 | 取值 |
|---|---|
| `pwc_version` | `0.2.1` |
| `provider` | `{"name":"PWC Provider","version":"<crate 版本>","platform":"desktop"}` |
| `profiles` | `pwc-core-read-v1`、`pwc-psbt-signer-v1` |
| `supported_networks` | `mainnet`、`signet`、`regtest` |
| `methods` | 第 3 节全部 24 个方法 |
| `features` | `coin_control`、`watch_only`、`event_push` |
| `scopes` | Core §6 全部 11 个 |
| `script_types` | `p2wpkh`、`p2pkh` |
| `message_formats` | `bip322-simple`、`legacy` |
| `limits.max_page_size` | 100 |
| `limits.max_psbt_bytes` | 100000 |
| `limits.max_combine_psbts` | 5 |
| `limits.max_raw_transaction_bytes` | 400000 |
| `limits.max_message_bytes` | 65536 |
| `limits.max_concurrent_operations` | 8 |
| `limits.operation_retention_seconds` | 86400 |
| `limits.high_fee_rate` | `"500000"`（500 sat/vB） |

## 3. 方法

| 方法 | scope | 返回 | 说明 |
|---|---|---|---|
| `pwc.getCapabilities` | — | 同步 | |
| `session.getInfo` | — | 同步 | |
| `session.requestPermissions` | — | Operation | 总是弹出审批提示，不走免提示捷径。不认识的钱包或账户 id 静默忽略 |
| `session.getPermissions` | — | 同步 | 返回全部授权，含已撤销和已过期的 |
| `session.revokePermissions` | — | 同步，幂等 | |
| `wallet.list` | `wallets:read` | 同步 | |
| `account.list` | `wallets:read` | 同步 | |
| `account.getBalance` | `balances:read` | 同步 | |
| `address.getReceiveAddress` | `addresses:read` | 同步 | 返回第一个未使用的接收地址 |
| `address.create` | `addresses:create` | 同步，幂等 | |
| `address.list` | `addresses:read` | 同步，分页 | |
| `utxo.list` | `utxos:read` | 同步，分页 | |
| `transaction.list` | `transactions:read` | 同步，分页 | 最新的在前 |
| `transaction.get` | `transactions:read` | 同步 | |
| `psbt.create` | `psbt:create` | Operation（同步完成） | 预留输入 10 分钟 |
| `psbt.releaseReservation` | `psbt:create` | 同步，幂等 | |
| `psbt.sign` | `psbt:sign` | Operation | 每笔交易单独审批 |
| `psbt.combine` | — | 同步 | |
| `psbt.finalize` | — | 同步 | |
| `transaction.broadcast` | `transactions:broadcast` | Operation（同步完成） | |
| `message.sign` | `messages:sign` | Operation | 每条消息单独审批 |
| `message.verify` | — | 同步 | |
| `operation.get` | — | 同步 | |
| `operation.cancel` | — | 同步，幂等 | |

各账户在 `AccountSummary.methods` 中列出的方法：

| 账户 | 不支持的方法 |
|---|---|
| P2WPKH 签名账户 | 无 |
| P2PKH 签名账户 | `psbt.create`、`psbt.releaseReservation`、`psbt.sign` |
| P2WPKH 观察账户（`can_sign: false`） | `psbt.sign`、`message.sign` |

对账户调用其不支持的方法，返回 `UNSUPPORTED_OPERATION`。

## 4. 规范留白处的取值

| 留白 | 取值 | 依据 |
|---|---|---|
| 参数中的未知成员 | 拒绝，`INVALID_ARGUMENT` | Core §3.1 |
| 锁定钱包上返回 Operation 的方法 | 直接 `WALLET_LOCKED`，不使用 `awaiting_unlock` | Core §8 |
| 手续费率超过 `high_fee_rate`、手续费无法确定、非默认 sighash | `POLICY_VIOLATION` | Core §8.3 |
| 低于最低费率 1000 sat/kvB | `POLICY_VIOLATION` | Core §3.4 |
| `wallet_estimate` | 后端按 `target_blocks` 给出费率；后端未配置估算时返回 `STATE_UNAVAILABLE` | Core §8.3 |
| 方法自身检查失败 | 直接返回错误，不创建 Operation。例外：广播被后端拒绝或后端不可达时，返回 `failed` 的 Operation；审批之后的失败也是 `failed` | Core §9.1 |
| 审批提示频率 | 每个客户端每 60 秒最多 10 次，在检查 9 判断 | Core §12 第 13 项 |
| 等待审批的时限 | 300 秒 | Core §9.1 |
| 分页游标 | 无状态，偏移量加 HMAC，绑定方法、账户、客户端；用错返回 `INVALID_ARGUMENT` | Core §8 |
| 已撤销和已过期的授权 | 一直保留，用于区分 `PERMISSION_REVOKED` | Core §8 检查 6 |
| 预留 | 实现，有效期 10 分钟；预留期间这些 UTXO 显示为 `locked` | Core §8.3 |
| 授权的 scope 为空 | 视为拒绝，Operation 进入 `rejected` | — |
| 用户选择的网络与请求不同 | 视为拒绝 | Core §8.1 |
| 加载快照时仍在等待审批的 Operation | `failed`，`INTERNAL_ERROR`，`retryable: true` | — |
| 授权被撤销或过期时未结束的 Operation | 立即进入 `failed`，`PERMISSION_REVOKED`，并撤回提示 | Core §7 第 6 步 |
| `local.unpair` 时未结束的 Operation | 同上，含 `session.requestPermissions` | LOCAL §5.7 |

## 5. 签名与消息

### 5.1 `psbt.sign` 的检查顺序

在 Core §8.3 规定的顺序之前，先检查账户是否支持 `psbt.sign`：

1. 账户不支持 `psbt.sign`：`UNSUPPORTED_OPERATION`
2. PSBT 版本不是 0：`UNSUPPORTED_PSBT_VERSION`；无法解析：`PSBT_VALIDATION_FAILED`
3. `input_indexes` 有重复或越界：`INVALID_ARGUMENT`
4. 目标输入不是本账户在本网络上的输出：`UNSUPPORTED_OPERATION`
5. 脚本不是 P2WPKH，或 sighash 不是标准值：`UNSUPPORTED_SCRIPT`
6. PSBT 中的前序输出与钱包记录不一致，或输出总额大于输入总额：`PSBT_VALIDATION_FAILED`
7. 净流出超过 `max_amount_sats_per_request`：`GRANT_CONSTRAINT_VIOLATED`
8. 费率过高、手续费无法确定、sighash 不是 `SIGHASH_ALL`：`POLICY_VIOLATION`

已带本账户签名的输入不再签名。只有这类输入时返回成功，`signed_input_indexes` 为空。

### 5.2 消息签名

| 格式 | 地址类型 | 签名编码 |
|---|---|---|
| `bip322-simple` | P2WPKH | BIP-322 simple：见证栈的共识编码，再做 base64 |
| `legacy` | P2PKH | 65 字节紧凑签名的 base64，前缀为 `Bitcoin Signed Message:\n` |

`message.sign` 的检查顺序：账户支持 → 地址可解析（否则 `INVALID_ARGUMENT`）→ 地址网络（否则 `NETWORK_MISMATCH`）→ 格式受支持且与地址类型匹配（否则 `UNSUPPORTED_OPERATION`）→ 地址属于本账户（否则 `INVALID_ARGUMENT`）。

`message.verify` 的检查顺序：网络受支持（否则 `UNSUPPORTED_OPERATION`）→ 地址可解析 → 地址网络 → 格式与地址类型 → 签名可解码（否则 `INVALID_ARGUMENT`）→ 返回 `valid`。

## 6. 演示数据

`MemoryBackend::demo()` 为每个网络 `<net>`（`mainnet`、`signet`、`regtest`）建立：

| 钱包 | 账户 | 脚本 | 可签名 | 预置余额（已确认） |
|---|---|---|---|---|
| `w-<net>-main`，软件钱包 | `a-<net>-wpkh` | P2WPKH | 是 | 1000000 sat 与 500000 sat 两个 UTXO |
| | `a-<net>-pkh` | P2PKH | 是 | 200000 sat |
| `w-<net>-watch`，观察钱包 | `a-<net>-watch` | P2WPKH | 否 | 300000 sat |

各网络的链高度为 1000，`wallet_estimate` 的费率估算为 2000 sat/kvB（可用 `MemoryBackend::set_fee_estimate` 修改）。密钥由固定种子派生，只用于测试。

## 7. 库接口

```rust
let (provider, prompts) = Provider::new(backend, Config::default());
let (provider, prompts) = Provider::with_state(backend, Config::default(), &snapshot)?;
let snapshot: Vec<u8> = provider.save_state();

let endpoint = local::bind(&dir, "pwc-provider")?;
local::serve(provider.clone(), endpoint).await;

provider.notify(BackendEvent::BalanceChanged { account_id });   // 宿主在钱包数据变化后调用
provider.revoke_grant(&grant_id);                                // 用户在钱包界面撤销授权
provider.unpair_client(&client_id);                              // 用户在钱包界面解除配对
provider.paired_clients();                                       // 配对列表（LOCAL §5.7）
```
