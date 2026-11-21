# TASK-001：工程脚手架与 schema 校验辅助

里程碑：M0

## 目标

建立 Rust 工程与测试辅助，能在测试里用 strict schema 校验任意输出。

## 先写的测试

- 加载 `pwc-schema/` 下 core strict 与 LOCAL schema，按 `$id` 注册，能解析跨文件 `$ref`
- 对一个已知合法和一个已知非法的 `Capabilities` 示例给出正确判断

## 实现内容

- 根目录 `.gitignore`（`target/`、`__pycache__/`）
- `pwc-provider/Cargo.toml`、空库
- `tests/common/mod.rs`：schema 校验器

## 验收标准

- `cargo test` 通过

## 覆盖的测试编号

—
