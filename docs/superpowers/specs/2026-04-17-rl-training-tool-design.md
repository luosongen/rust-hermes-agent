# RL Training Tool 设计文档

> **Status:** Approved for implementation

## 1. 概述

**目标：** 实现 RL Training Tool，通过 tinker-atropos 子模块提供强化学习训练能力。

**技术方案：** 新建 `RLTrainingTool`，通过 `git submodule` 集成 tinker-atropos，使用 `std::process::Command` 调用训练脚本。

---

## 2. tinker-atropos 集成

### Submodule 添加

```bash
git submodule add git@github.com:.../tinker-atropos.git third-party/tinker-atropos
```

### 目录结构

```
rust-hermes-agent/
├── third-party/
│   └── tinker-atropos/
│       ├── src/
│       ├── scripts/
│       │   └── train.py       # 训练入口脚本
│       └── requirements.txt
```

### 依赖安装

Tool 初始化时检查 Python 依赖：
```bash
pip install -r third-party/tinker-atropos/requirements.txt
```

---

## 3. 数据结构

### TrainingConfig

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingConfig {
    pub model: String,           // 模型路径或名称
    pub dataset: String,         // 数据集路径
    pub epochs: Option<u32>,     // 训练轮数，默认 100
    pub batch_size: Option<u32>,
    pub learning_rate: Option<f64>,
    pub output_dir: Option<String>,
    #[serde(default)]
    pub extra_args: std::collections::HashMap<String, String>,
}
```

### TrainingStatus

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingStatus {
    pub id: String,
    pub config: TrainingConfig,
    pub state: TrainingState,
    pub started_at: f64,
    pub updated_at: f64,
    pub progress: Option<Progress>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum TrainingState {
    Pending,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Progress {
    pub current_epoch: u32,
    pub total_epochs: u32,
    pub loss: Option<f64>,
    pub metrics: std::collections::HashMap<String, f64>,
}
```

### TrainingResult

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingResult {
    pub id: String,
    pub model_path: String,
    pub final_loss: Option<f64>,
    pub metrics: std::collections::HashMap<String, f64>,
    pub completed_at: f64,
    pub duration_secs: u64,
}
```

---

## 4. Tool Actions

### start_training

**参数：**
```json
{
  "action": "start_training",
  "config_path": "/path/to/config.json",
  "model": "gpt2",
  "dataset": "/path/to/dataset.jsonl"
}
```

**流程：**
1. 生成唯一 `training_id` (UUID)
2. 创建目录 `~/.config/hermes-agent/rl_training/{training_id}/`
3. 保存 `config.json`
4. 生成 `status.json` (state: Starting)
5. 启动后台进程: `python train.py --config {training_id}/config.json`
6. 更新状态为 Running
7. 返回 `training_id`

**返回：**
```json
{
  "status": "ok",
  "training_id": "uuid-xxx",
  "state": "Starting"
}
```

### get_status

**参数：** `training_id`

**返回：**
```json
{
  "id": "uuid-xxx",
  "state": "Running",
  "progress": {
    "current_epoch": 45,
    "total_epochs": 100,
    "loss": 0.234,
    "metrics": { "accuracy": 0.89 }
  }
}
```

### list_trainings

**参数：** 无

**返回：**
```json
{
  "trainings": [
    {
      "id": "uuid-xxx",
      "model": "gpt2",
      "state": "Running",
      "started_at": 1710000000.0
    }
  ]
}
```

### cancel_training

**参数：** `training_id`

**流程：**
1. 读取 `status.json`
2. 发送 SIGTERM 到训练进程
3. 更新状态为 Cancelled
4. 保留输出目录

**返回：**
```json
{
  "status": "ok",
  "training_id": "uuid-xxx",
  "state": "Cancelled"
}
```

### get_results

**参数：** `training_id`

**前置条件：** state == Completed

**返回：**
```json
{
  "id": "uuid-xxx",
  "model_path": "~/.config/hermes-agent/rl_training/uuid-xxx/model",
  "final_loss": 0.123,
  "metrics": { "accuracy": 0.95, "f1": 0.92 },
  "completed_at": 1710003600.0,
  "duration_secs": 3600
}
```

---

## 5. 目录结构

```
~/.config/hermes-agent/rl_training/
├── {training_id}/
│   ├── config.json       # 训练配置
│   ├── status.json       # 当前状态
│   ├── model/            # 输出模型
│   └── logs/             # 训练日志
└── latest -> {training_id}  # 最新训练的软链接
```

---

## 6. 进程管理

- 训练作为后台 `tokio::process::Command` 运行
- 状态文件作为进程间通信机制
- 定期读取 `status.json` 更新进度
- 进程退出时更新最终状态

---

## 7. 错误处理

| 场景 | 处理 |
|------|------|
| tinker-atropos 未初始化 | 返回错误，提示 `git submodule update --init` |
| Python 依赖缺失 | 尝试自动安装，失败则报错 |
| 训练进程崩溃 | 读取 status.json，更新为 Failed |
| training_id 不存在 | 返回错误 |
| 磁盘空间不足 | 提前检查，返回错误 |

---

## 8. 文件变更

- **创建：** `crates/hermes-tools-extended/src/rl_training.rs` — 主模块
- **创建：** `crates/hermes-tools-extended/src/rl_training/` — training 子模块目录
- **创建：** `crates/hermes-tools-extended/tests/test_rl_training.rs` — 测试
- **修改：** `crates/hermes-tools-extended/src/lib.rs` — 导出 RLTrainingTool
- **修改：** `Cargo.toml` — 添加 git submodule 依赖说明

---

## 9. 验收标准

- [ ] `start_training` 能启动训练并返回 training_id
- [ ] `get_status` 能返回实时进度
- [ ] `list_trainings` 能列出所有训练
- [ ] `cancel_training` 能停止运行中的训练
- [ ] `get_results` 能在训练完成后返回结果
- [ ] 训练状态正确持久化到 status.json
- [ ] 进程崩溃时状态正确更新为 Failed
