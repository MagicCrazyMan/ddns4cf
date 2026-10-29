# 介绍

这是一个专为 Cloudflare 设计的 DDNS 服务，通过 Cloudflare API 定时更新运行该软件的设备的公网 IP 地址。

该程序功能比较简单，当前仅支持更新 IP 的功能。（IPv4 可用，IPv6 未经测试，理论上可行）

# Docker

在宿主机使用 musl 静态编译后构建镜像。运行时镜像固定使用 `debian:13` 作为基底；构建脚本会识别 `x86_64` 和 `aarch64` 宿主机架构，选择对应的 Rust musl target。若未安装 musl 工具链，脚本会提示安装命令。

```bash
chmod +x docker/build-image.sh
docker/build-image.sh
```

镜像会同时标记为 `ddns4f:<Cargo.toml 中的 version>` 和 `ddns4f:latest`。

配置文件通过挂载提供，容器会读取 `/config/config.toml`：

```bash
docker run -d --name ddns4cf \
  --restart unless-stopped \
  -v "$(pwd)/config.toml:/config/config.toml:ro" \
  ddns4f:0.3.19
```

也可使用 Compose 模板（默认使用 GHCR 的 `latest` 镜像）。将配置文件放在 `docker/config.toml` 后执行：

```bash
cd docker
docker compose up -d
```

## GitHub Actions

推送以 `v` 开头的标签（例如 `v0.3.19`）会触发 [`.github/workflows/docker-publish.yml`](.github/workflows/docker-publish.yml)：分别构建 `linux/amd64` 和 `linux/arm64` 静态镜像，合并为多架构镜像后发布到 GHCR。arm64 构建使用 GitHub 托管的 `ubuntu-24.04-arm` runner。

发布标签：

- `ghcr.io/<owner>/ddns4f:<去掉 v 的标签>`
- `ghcr.io/<owner>/ddns4f:latest`

```bash
docker pull ghcr.io/<owner>/ddns4f:latest
```

# 配置

配置文件使用 `toml` 格式，可通过 `-c/--config` 参数指定路径；未指定时读取可执行文件同目录下的 `config.toml`。

完整配置模板见 [`config.example.toml`](config.example.toml)，复制为 `config.toml` 后按文件内注释修改即可，各字段的含义、默认值与写法均在模板中说明。

# 独立服务器

若使用独立服务器作为 IP 来源，程序会向目标 URL 发送一个 `GET` 请求。目标服务器应当返回响应类型为 `text/plain` 的结果，其中直接携带对应的 IP 地址即可。
