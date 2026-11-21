# TASK-002：错误码、语义类型、手续费与认证证明

里程碑：M1

## 目标

实现 Core §3.2 的语义类型校验、§10 的错误码、§3.4 的手续费公式，以及 LOCAL §5.4 的 HMAC 证明。

## 先写的测试

- `vectors/types.json` 中每个原子类型向量
- `vectors/misc.json` 中 FEE-01 到 FEE-07
- LAUTH-01 两个证明

## 实现内容

- `src/error.rs`
- `src/types.rs`
- `local::client_proof`、`local::server_proof`

## 验收标准

- 上述向量全部通过；复合类型向量的跳过清单与预期一致

## 覆盖的测试编号

C-22、C-23、C-26、C-29、L-12
