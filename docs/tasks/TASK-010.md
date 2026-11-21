# TASK-010：状态保存与加载

里程碑：M9

## 目标

提供 `save_state` 与 `with_state`，由宿主钱包调用。

## 先写的测试

- 保存后用新 Provider 加载：旧凭据可直接认证
- 授权仍有效
- 幂等重试返回原响应
- 已完成 Operation 可查
- 预留仍生效
- 等待审批的 Operation 变为 `failed`/`INTERNAL_ERROR`
- 损坏或版本不符的快照报错

## 实现内容

- `Provider::save_state`
- `Provider::with_state`

## 验收标准

- 上述测试通过

## 覆盖的测试编号

Core §8.5、§9.3，LOCAL §5.5
