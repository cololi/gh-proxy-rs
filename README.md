# Hub-Proxy (Rust)

GitHub 和 Hugging Face 加速代理。支持 Git Clone、Release、Blob 以及大文件下载加速。Rust 实现版本。

## 特点

- **双平台支持** — 同时支持 GitHub 和 Hugging Face (模型、数据集、Spaces)
- **简单部署** — 支持 Docker、二进制运行以及 systemd (用户态)
- **自动转换** — 自动将 GitHub 的 `blob` 预览链接转换为 `raw` 直链下载
- **静态二进制** — 基于 musl 构建，无外部运行时依赖

## 性能表现

本项目采用 **流式转发 (Streaming)** 技术，基于 **Rust + Tokio 异步运行时**、**rustls TLS**，零成本抽象，实现了极低的性能开销：
- **内存占用**：典型负载下 RSS 占用极低，无 GC 抖动。
- **并发能力**：依托 Tokio 异步 I/O，能够处理大量并发下载请求。
- **低延迟**：不进行磁盘缓存，数据在内存中直接透传，响应速度极快。
- **安全可控**：纯 rustls 实现 TLS，无 OpenSSL/glibc 依赖。

## 快速开始

### 1. 使用 Docker

镜像发布在 GHCR 和 Docker Hub：

**Docker Hub:**
```bash
docker run -d --name hub-proxy -p 8080:8080 --restart always cololi/hub-proxy:latest
```

**GHCR:**
```bash
docker run -d --name hub-proxy -p 8080:8080 --restart always ghcr.io/cololi/hub-proxy-rs:latest
```

### 2. 使用 systemd (Linux 推荐)

<details>
<summary><b>一键安装脚本 (推荐 - 用户态)</b></summary>

```bash
curl -sSL https://raw.githubusercontent.com/cololi/hub-proxy-rs/master/scripts/install.sh | bash
```
</details>

<details>
<summary><b>手动安装 (用户态)</b></summary>

1. 下载或编译 `hub-proxy` 二进制文件至用户目录：
   ```bash
   make build
   mkdir -p ~/.local/bin
   cp hub-proxy ~/.local/bin/
   ```
2. 创建用户服务文件 `~/.config/systemd/user/hub-proxy.service`：
   ```ini
   [Unit]
   Description=Hub-Proxy Service
   After=network.target

   [Service]
   ExecStart=%h/.local/bin/hub-proxy
   Restart=always
   Environment=LISTEN=:8080

   [Install]
   WantedBy=default.target
   ```
3. 启动并启用服务：
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable --now hub-proxy
   ```

**查看日志:**
```bash
journalctl --user -u hub-proxy -f
```

**持久化运行:**
执行以下命令确保用户注销后服务继续运行：
```bash
sudo loginctl enable-linger $(whoami)
```
</details>

### 3. 本地编译运行

<details>
<summary><b>展开查看本地运行步骤</b></summary>

使用 Make：
```bash
make run
```

或直接使用 Cargo：
```bash
cargo build --release
./target/release/hub-proxy
```
</details>

## 配置说明 (环境变量)

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `LISTEN` | `:8080` | 监听地址 |
| `SIZE_LIMIT` | `1072668082176` | 文件大小限制，超出则 302 跳转到原始地址 |
| `BUFFER_SIZE` | `32768` | 流式转发缓冲区大小 (字节) |
| `UPSTREAM_TIMEOUT` | `30s` | 上游连接超时时间 |
| `SHUTDOWN_TIMEOUT` | `10s` | 优雅停机超时时间 |

## 使用示例

### GitHub 加速
```bash
# Git 克隆
git clone https://你的域名/https://github.com/user/repo
```

### Hugging Face 加速
```bash
# Git 克隆模型
git clone https://你的域名/https://huggingface.co/gpt2
```
