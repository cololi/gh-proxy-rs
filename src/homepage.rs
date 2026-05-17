//! Static HTML for the landing page.
//!
//! Verbatim copy of the `defaultIndexHTML` constant in
//! `internal/proxy/proxy.go`. The `{{.Origin}}` literal is consumed by the
//! client-side JS at runtime, so it must survive unmodified — hence the raw
//! string literal.

pub const HOMEPAGE: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Hub-Proxy-Go | 高性能 GitHub/HuggingFace 加速</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; max-width: 800px; margin: 80px auto; padding: 0 20px; color: #24292e; line-height: 1.6; background-color: #f6f8fa; }
        .container { background: #fff; padding: 40px; border-radius: 8px; box-shadow: 0 1px 3px rgba(27,31,35,0.12), 0 1px 2px rgba(27,31,35,0.24); }
        h1 { margin-top: 0; font-size: 28px; font-weight: 600; color: #0366d6; }
        p { color: #586069; margin-bottom: 30px; }
        .input-group { display: flex; gap: 10px; margin-bottom: 30px; }
        input { flex: 1; padding: 12px 16px; font-size: 16px; border: 1px solid #d1d5da; border-radius: 6px; outline: none; transition: border-color 0.2s, box-shadow 0.2s; }
        input:focus { border-color: #0366d6; box-shadow: 0 0 0 3px rgba(3,102,214,0.3); }
        button { padding: 12px 24px; font-size: 16px; font-weight: 600; color: #fff; background-color: #2ea44f; border: 1px solid rgba(27,31,35,0.15); border-radius: 6px; cursor: pointer; transition: background-color 0.2s; }
        button:hover { background-color: #2c974b; }
        h2 { font-size: 18px; font-weight: 600; margin-top: 30px; border-bottom: 1px solid #eaecef; padding-bottom: 8px; }
        pre { background: #f6f8fa; padding: 16px; border-radius: 6px; font-size: 14px; overflow-x: auto; color: #444; border: 1px solid #dfe1e4; margin-top: 10px; }
        code { font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace; }
        ul { padding-left: 20px; color: #586069; }
        li { margin-bottom: 8px; }
        .footer { margin-top: 40px; text-align: center; font-size: 12px; color: #6a737d; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Hub-Proxy-Go</h1>
        <p>高性能透明代理，支持 GitHub (Releases, Archive, Blob, Raw) 和 Hugging Face 资源加速。</p>

        <form method="get" action="/">
            <div class="input-group">
                <input name="q" placeholder="输入 GitHub 或 Hugging Face 链接..." autofocus required>
                <button type="submit">立即加速</button>
            </div>
        </form>

        <h2>支持的格式示例</h2>
        <pre><code># GitHub Release / 源码包
https://github.com/user/repo/releases/download/v1.0/file.zip
https://github.com/user/repo/archive/refs/heads/main.zip

# GitHub 文件 (自动转换为 Raw)
https://github.com/user/repo/blob/main/README.md

# Hugging Face 模型/数据集
https://huggingface.co/gpt2/resolve/main/config.json
https://huggingface.co/datasets/user/data/resolve/main/file.csv

# Git 克隆加速
git clone {{.Origin}}/https://github.com/user/repo.git</code></pre>

        <h2>使用说明</h2>
        <ul>
            <li>直接在输入框粘贴链接并点击“立即加速”。</li>
            <li>对于 Git 克隆，在原链接前加上本站地址。</li>
            <li>本项目仅用于合规的学习与研究目的，请勿用于非法用途。</li>
        </ul>
    </div>
    <div class="footer">
        Powered by Hub-Proxy-Go
    </div>
    <script>
        // 自动替换 Origin 占位符
        document.body.innerHTML = document.body.innerHTML.replace(/{{.Origin}}/g, window.location.origin);
    </script>
</body>
</html>"#;
