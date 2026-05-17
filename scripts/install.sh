#!/bin/bash
set -e

REPO="cololi/hub-proxy-rs"
BINARY_NAME="hub-proxy"

# 检查依赖
if ! command -v curl >/dev/null 2>&1; then
    echo "错误: 未安装 curl。"
    exit 1
fi

# 检测架构
ARCH=$(uname -m)
case $ARCH in
    x86_64)
        FILE_ARCH="linux-amd64"
        ;;
    aarch64|arm64)
        FILE_ARCH="linux-arm64"
        ;;
    *)
        echo "不支持的架构: $ARCH"
        exit 1
        ;;
esac

echo "正在检测最新版本..."
LATEST_TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    echo "错误: 无法找到最新版本。"
    exit 1
fi

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/${BINARY_NAME}-${FILE_ARCH}"

# 创建用户级二进制目录
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

echo "正在下载 ${BINARY_NAME} ${LATEST_TAG} (${FILE_ARCH})..."
curl -L -o "$BIN_DIR/${BINARY_NAME}" "$DOWNLOAD_URL"
chmod +x "$BIN_DIR/${BINARY_NAME}"

# 创建用户级 systemd 配置目录
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"
mkdir -p "$SYSTEMD_USER_DIR"

echo "正在创建用户级 systemd 服务..."
cat > "$SYSTEMD_USER_DIR/${BINARY_NAME}.service" <<EOF
[Unit]
Description=Hub-Proxy Service
After=network.target

[Service]
ExecStart=$BIN_DIR/${BINARY_NAME}
Restart=always
Environment=LISTEN=:8080

[Install]
WantedBy=default.target
EOF

echo "正在启动服务 (用户态)..."
systemctl --user daemon-reload
systemctl --user enable --now ${BINARY_NAME}

echo "------------------------------------------------"
echo "成功安装 ${BINARY_NAME} 至用户态！"
echo "二进制路径: $BIN_DIR/${BINARY_NAME}"
echo "服务状态: "
systemctl --user status ${BINARY_NAME} --no-pager
echo "------------------------------------------------"
echo "提示: 确保您的用户已启用 lingering 以便在注销后保持运行:"
echo "sudo loginctl enable-linger \$(whoami)"
