# 介绍

这是一个专为 Cloudflare 设计的 DDNS 服务，通过 Cloudflare API 定时更新运行该软件的设备的公网 IP 地址。

该程序功能比较简单，当前仅支持更新 IP 的功能。（IPv4 可用，IPv6 未经测试，理论上可行）

# 配置

支持从外部文件读取配置信息(`-c` 参数)，格式要求为 `json` 或 `json5`。配置详情如下：

- `Configuration`

  | 字段           | 类型                                    | 介绍                                                         |
  | -------------- | --------------------------------------- | ------------------------------------------------------------ |
  | fresh_interval | `number` | `undefined`                  | 全局刷新间隔，单位秒。默认为 900 秒                          |
  | retry_interval | `number` | `undefined`                  | 全局出现错误时重试间隔，单位秒。默认为 300 秒                |
  | ip_source      | `0` | `1` | `[1, string]` | `undefined` | 全局 IP 地址来源。默认为 `0`<br />- `0`：通过 `IpIp` 网页获取<br />- `1`：通过[独立服务器](#独立服务器)获取 |
  | accounts       | `Account[]`                             | Cloudflare 账户列表                                          |

  

- `Account`

  | 字段    | 类型       | 介绍                                                         |
  | ------- | ---------- | ------------------------------------------------------------ |
  | token   | `string`   | Cloudflare API token<br />为保证安全，仅可通过 token 访问 API，不支持使用账户密码 |
  | domains | `Domain[]` | 当前账户下的域名记录                                         |

  

- `Domain`

  | 字段           | 类型                                    | 介绍                                                         |
  | -------------- | --------------------------------------- | ------------------------------------------------------------ |
  | fresh_interval | `number` | `undefined`                  | 刷新间隔，单位秒。<br />若配置该项，则不会使用全局刷新间隔   |
  | retry_interval | `number` | `undefined`                  | 出现错误时重新间隔，单位秒<br />若配置该项，则不会使用全局重试间隔 |
  | ip_source      | `0` | `1` | `[1, string]` | `undefined` | IP 地址来源<br />若配置该项，则不会使用全局 IP 地址来源      |
  | nickname       | `string`                                | 域名昵称，用于输出日志                                       |
  | id             | `string`                                | Cloudflare 中当前域名记录的 id                               |
  | zone_id        | `string`                                | Cloudflare 中当前域名记录的 zone id                          |



## 示例

```json5
{
  fresh_interval: 900,
  retry_interval: 600,
  ip_source: 1,
  accounts: [
    {
      // Cloudflare API token，注意保护 token 安全
      token: "token",
      domains: [
        {
          // 该域名将会使用 1200 秒作为刷新间隔，而非 900 秒
          fresh_interval: 1200,
          // 该域名将会使用 300 秒作为重试间隔，而非 600 秒
          retry_interval: 300,
          nickname: "test",
          id: "record_id",
          zone_id: "zone_id",
          // 该域名将会使用独立服务器作为 IP 来源，而非 IPIP
          // 且将会访问 `http://127.0.0.1:8000/ip` 地址
          ip_source: [1, "http://127.0.0.1:8000/ip"],
        },
      ],
    },
  ],
}

```



# 独立服务器

若使用独立服务器作为 IP 来源，程序会向目标 URL 发送一个 `GET` 请求。目标服务器应当返回响应类型为 `text/plain` 的结果，其中直接携带对应的 IP 地址即可。