# TASK-003：LOCAL 传输与握手

里程碑：M2

## 目标

实现端点、对端检查、帧、JSON-RPC 信封、`local.hello`、`local.pair`、`local.authenticate`。

## 先写的测试

- 端点目录权限不对时拒绝监听；路径超长；残留 socket 与运行中实例
- 首个请求不是 hello；未知协议；hello 与认证超时
- 配对批准、拒绝、超时、连接关闭时撤回且不建凭据；提示内容含未验证标注和对端信息
- 错误证明与未知 client_id 的响应完全相同且连接关闭；重放证明失败
- 超长帧、非 JSON、批量、重复的未完成 id

## 实现内容

- `src/local.rs`
- `src/prompt.rs` 的配对部分
- Provider 的客户端表

## 验收标准

- 上述测试通过

## 覆盖的测试编号

L-01、L-03 到 L-11、L-14 到 L-17、L-25 到 L-27
