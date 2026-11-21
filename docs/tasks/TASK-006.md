# TASK-006：psbt.combine 与 psbt.finalize

里程碑：M5

## 目标

实现纯数据的 PSBT 合并与最终化。

## 先写的测试

- `vectors/psbt.json` PSBT-01 到 PSBT-08（按键值集合比较）
- 越界 vout 不崩溃
- 超过 `max_psbt_bytes` 与 `max_combine_psbts`

## 实现内容

- `src/psbt.rs` 的原始解析、combine、finalize
- 两个方法的处理

## 验收标准

- 上述测试通过

## 覆盖的测试编号

C-11 到 C-14、C-30、C-40、C-55
