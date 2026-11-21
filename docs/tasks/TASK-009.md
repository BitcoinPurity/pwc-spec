# TASK-009：事件推送

里程碑：M8

## 目标

实现 `pwc.event` 推送与过滤，声明 `event_push`。

## 先写的测试

- 有无 `events:read` 时的投递差别
- 未授权的监听者收不到受保护信息
- 锁定或不可用钱包不发 `account.*` 与 `wallet.syncChanged`
- 客户端不读导致缓冲超限时连接被关闭

## 实现内容

- `Provider::notify`
- 事件扇出与过滤

## 验收标准

- 上述测试通过

## 覆盖的测试编号

C-27、C-53、L-21、L-22
