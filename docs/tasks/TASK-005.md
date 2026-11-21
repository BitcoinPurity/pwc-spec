# TASK-005：后端 trait 与读方法

里程碑：M4

## 目标

定义 `WalletBackend`，实现内存后端的数据与读路径，以及全部读方法和分页。

## 先写的测试

- `vectors/balance.json` BAL-01 到 BAL-05
- 只返回已授权账户的数据
- 锁定钱包与不可用钱包
- 授权外与不存在的账户响应完全相同
- 游标换账户使用得到 `INVALID_ARGUMENT`
- 钱包新增账户自动被授权覆盖
- 重组后确认数重新计算
- 替换交易不重复计入
- 未知 txid 得到 `TRANSACTION_NOT_FOUND`
- 三个网络的钱包按授权网络隔离
- `address.create` 幂等

## 实现内容

- `src/backend.rs`
- `src/balance.rs`
- `src/memory.rs` 的读路径
- 读方法处理

## 验收标准

- 上述测试通过

## 覆盖的测试编号

C-02、C-16、C-17、C-24、C-25、C-34、C-35、C-36、C-38、C-57、C-58
