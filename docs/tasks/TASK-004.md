# TASK-004：会话、授权、Operation、幂等与检查管线

里程碑：M3

## 目标

实现方法表、十步检查管线、`pwc.getCapabilities`、`session.*`、`operation.*`、`local.unpair`、演示程序。

## 先写的测试

- 无授权枚举钱包得到 `PERMISSION_REQUIRED`
- 幂等精确重试返回同一 Operation；改参数得到 `IDEMPOTENCY_CONFLICT`
- 跨客户端访问 Operation 得到 `OPERATION_NOT_FOUND`
- 终态 Operation 不再变化
- 授权在 `valid_until` 过期
- 授权结果不超出请求
- 等待审批到期变为 `expired`
- 超出并发上限得到 `RATE_LIMITED`
- 被拒后频繁请求得到 `RATE_LIMITED`
- 撤销他人授权得到 `GRANT_NOT_FOUND`
- 检查顺序专项：同时触发多个错误时返回排在前面的
- 启动演示程序跑 `local_runner.py --pair --allow-destructive`，无失败项

## 实现内容

- `src/methods.rs`
- `src/provider/`
- `src/main.rs`

## 验收标准

- 上述测试通过；runner 无失败项

## 覆盖的测试编号

C-01、C-05、C-18、C-20、C-31、C-33、C-37、C-50、C-51、C-52、C-56、C-62、L-18、L-19、L-20、L-23
