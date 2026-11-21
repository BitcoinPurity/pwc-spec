# TASK-008：psbt.sign 与消息签名

里程碑：M7

## 目标

实现签名前审查、审批后签名、BIP-322 simple 与 legacy 消息签名。

## 先写的测试

- 签名等待中撤销授权得到 `failed`/`PERMISSION_REVOKED`
- 观察账户签名得到 `UNSUPPORTED_OPERATION`
- 改 PSBT 重用幂等键
- 前序输出金额不一致得到 `PSBT_VALIDATION_FAILED`
- 高费率或非默认 sighash 得到 `POLICY_VIOLATION`
- 部分签名不最终化
- 只签 `[0]`
- `input_indexes` 为空、重复、越界
- 指定不可签的输入
- 已签过的输入
- 净流出超限且不弹提示
- 净流出为负不触发限额
- 其他网络的输入
- 非 ASCII 与尾随空白的消息签名后可验证
- BIP-322 官方向量
- 不支持的网络验证消息
- 撤销授权后精确重试返回不带 result 的 Operation
- 端到端：create → sign → finalize → broadcast，余额随之变化

## 实现内容

- `psbt::review`
- 后端签名
- 审批任务
- `src/message.rs`

## 验收标准

- 上述测试通过

## 覆盖的测试编号

C-03、C-04、C-06 到 C-10、C-32、C-41 到 C-43、C-45、C-59 到 C-61、C-63
