use std::{borrow::Cow, env, fs, net::IpAddr, path::Path, sync::Arc};

use reqwest::{Client, Url};
use serde::{
    de::{self, Visitor},
    Deserialize,
};
use smallvec::SmallVec;
use tokio::sync::Mutex;

use super::{
    args,
    error::Error,
    source::{ipip::IpIp, standalone::Standalone, IpSource},
    updater::Updater,
};

/// 默认刷新间隔
const DEFAULT_FRESH_INTERVAL_SECONDS: u64 = 15 * 60;
/// 默认全局出现错误时重试间隔
const DEFAULT_RETRY_INTERVAL_SECONDS: u64 = 5 * 60;

/// 配置内容数据结构
///
/// 包含全局参数及需要刷新的域名列表。
#[derive(serde::Deserialize, Debug, Clone)]
pub struct Configuration {
    /// 绑定的本地 IP 地址，可选
    bind_address: Option<IpAddr>,
    /// 全局刷新间隔，单位秒。默认为 900 秒。
    ///
    /// 若通过 [`Domain`] 为单独的域名设置 `fresh_interval` 属性，该属性将不会被使用。
    fresh_interval: Option<u64>,
    /// 全局出现错误时重试间隔，单位秒。默认为 300 秒。
    ///
    /// 若通过 [`Domain`] 为单独的域名设置 `retry_interval` 属性，该属性将不会被使用。
    retry_interval: Option<u64>,
    /// 全局 IP 地址来源。默认为 `0`
    ///
    /// - `0`：IpIp
    /// - `1`：独立服务器
    /// - `2`：基于 Linux ip 命令查询（仅限 linux 系统）
    ip_source: Option<IpSourceType>,
    /// Cloudflare 账号列表
    accounts: Vec<Account>,
    /// Cloudflare 访问代理，可选。默认使用当前系统配置的全局代理
    proxy: Option<Proxy>,
    // /// 日志
    // log: Option<Log>,
}

impl Configuration {
    /// 获取绑定的本地 IP 地址
    pub fn bind_address(&self) -> Option<IpAddr> {
        self.bind_address
    }

    /// 获取全局刷新间隔，单位秒。默认为 900 秒。
    pub fn fresh_interval(&self) -> u64 {
        self.fresh_interval
            .unwrap_or(DEFAULT_FRESH_INTERVAL_SECONDS)
    }

    /// 获取 Cloudflare 账号列表
    pub fn accounts(&self) -> &[Account] {
        self.accounts.as_ref()
    }

    /// 获取 IP 来源方式
    pub fn ip_source_type(&self) -> &IpSourceType {
        self.ip_source.as_ref().unwrap_or(&IpSourceType::IpIp)
    }

    // 创建 Cloudflare HTTP reqwest client.
    fn create_cf_http_client(&self) -> Client {
        let mut builder = reqwest::ClientBuilder::new().local_address(self.bind_address);
        if let Some(proxy) = self.proxy() {
            builder = builder.proxy(proxy);
        };

        builder.build().unwrap()
    }

    /// 通过当前配置内容创建 [`Updater`] 列表
    pub fn create_updaters(&self) -> SmallVec<[Arc<Mutex<Updater>>; 4]> {
        let cf_http_client = self.create_cf_http_client();

        let mut updaters = SmallVec::new();
        self.accounts().iter().for_each(|account| {
            account.domains().iter().for_each(|domain| {
                let updater = Updater::new(
                    domain.bind_address().or(self.bind_address()),
                    domain
                        .ip_source()
                        .unwrap_or(self.ip_source_type())
                        .to_ip_source(),
                    domain.nickname(),
                    account.token(),
                    domain.id(),
                    domain.zone_id(),
                    domain.fresh_interval().unwrap_or(self.fresh_interval()),
                    domain.retry_interval().unwrap_or(self.retry_interval()),
                    cf_http_client.clone(),
                );

                updaters.push(Arc::new(Mutex::new(updater)));
            })
        });

        updaters
    }

    /// 获取全局出现错误时重试间隔，单位秒。默认为 300 秒后。
    pub fn retry_interval(&self) -> u64 {
        self.retry_interval
            .unwrap_or(DEFAULT_RETRY_INTERVAL_SECONDS)
    }

    /// 获取 Cloudflare 访问代理配置
    pub fn proxy(&self) -> Option<reqwest::Proxy> {
        // let Some(proxy) = &self.proxy else {
        //     return None;
        // };

        // let proxy = reqwest::Proxy::https(proxy.url.as_str()).

        self.proxy.as_ref().and_then(|proxy| Some(proxy.0.clone()))
    }

    // /// 获取日志参数
    // pub fn log(&self) -> Option<&Log> {
    //     self.log.as_ref()
    // }
}

/// 可用的 IP 地址来源方式
///
/// - `0`：IpIp
/// - `1`：独立服务器
/// - `2`：基于 Linux ip 命令查询（仅限 linux 系统）
#[derive(Debug, Clone)]
pub enum IpSourceType {
    IpIp,
    Standalone(Url),
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    LocalIPv6(Option<String>),
}

impl IpSourceType {
    fn to_ip_source(&self) -> Box<dyn IpSource> {
        match self {
            IpSourceType::IpIp => Box::new(IpIp::new()),
            IpSourceType::Standalone(socket_addr) => Box::new(Standalone::new(socket_addr.clone())),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            IpSourceType::LocalIPv6(interface_name) => {
                Box::new(super::source::local_ipv6::LocalIPv6::new(
                    interface_name.clone().map(|name| Cow::Owned(name)),
                ))
            }
        }
    }
}

impl<'de> Deserialize<'de> for IpSourceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IpSourceTypeVisitor;
        impl<'de> de::Visitor<'de> for IpSourceTypeVisitor {
            type Value = IpSourceType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                formatter.write_str(
                    "可用的 IP 地址来源方式为：0(IpIp)、 1(独立服务器) 或 2(Local IPv6)",
                )?;
                #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                formatter.write_str("可用的 IP 地址来源方式为：0(IpIp) 或 1(独立服务器)")?;

                Ok(())
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    0 => Ok(IpSourceType::IpIp),
                    1 => Err(E::custom(
                        "IP 来源方式 1(独立服务器) 必须指定服务器访问地址",
                    )),
                    #[cfg(any(target_os = "linux", target_os = "windows"))]
                    2 => Ok(IpSourceType::LocalIPv6(None)),
                    _ => Err(E::custom(format!("不支持的 IP 来源方式：{}", v))),
                }
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut r#type = None;
                let mut server = None;
                let mut interface = None;

                while let Some(key) = map.next_key::<Cow<'_, str>>()? {
                    match &*key {
                        "type" => r#type = Some(map.next_value::<i64>()?),
                        "server" => server = Some(map.next_value::<Cow<'_, str>>()?),
                        "interface" => interface = Some(map.next_value::<Cow<'_, str>>()?),
                        _ => {}
                    }
                }

                let Some(r#type) = r#type else {
                    return Err(de::Error::missing_field("type"));
                };

                match r#type {
                    0 => Ok(IpSourceType::IpIp),
                    1 => match server {
                        Some(server) => {
                            let Ok(server) = server.parse::<Url>() else {
                                return Err(de::Error::custom(format!(
                                    "无效服务器地址：{}",
                                    server
                                )));
                            };
                            Ok(IpSourceType::Standalone(server))
                        }
                        None => Err(de::Error::custom(
                            "IP 来源方式 1(独立服务器) 必须指定服务器访问地址",
                        )),
                    },
                    #[cfg(any(target_os = "linux", target_os = "windows"))]
                    2 => Ok(IpSourceType::LocalIPv6(
                        interface.map(|name| name.to_string()),
                    )),
                    _ => Err(de::Error::custom(format!(
                        "不支持的 IP 来源方式：{}",
                        r#type
                    ))),
                }
            }
        }

        deserializer.deserialize_any(IpSourceTypeVisitor)
    }
}

/// Cloudflare 账号数据
#[derive(serde::Deserialize, Debug, Clone)]
pub struct Account {
    /// Cloudflare 账号 API token
    token: String,
    /// Cloudflare 中需要刷新的域名列表
    domains: Vec<Domain>,
}

impl Account {
    /// 获取 Cloudflare 账号 token
    pub fn token(&self) -> &str {
        self.token.as_ref()
    }

    /// 获取 Cloudflare 中需要刷新的域名列表
    pub fn domains(&self) -> &[Domain] {
        self.domains.as_ref()
    }
}

/// Cloudflare 域名数据
#[derive(serde::Deserialize, Debug, Clone)]
pub struct Domain {
    /// 绑定的本地 IP 地址，可选
    bind_address: Option<IpAddr>,
    /// 刷新间隔，单位秒。
    ///
    /// 若未配置该项，则会使用 [`Configuration`] 中 `fresh_interval` 属性。
    fresh_interval: Option<u64>,
    /// 出现错误时重新间隔，单位秒。
    ///
    /// 若未配置该项，则会使用 [`Configuration`] 中 `retry_interval` 属性。
    retry_interval: Option<u64>,
    /// 当前机器运行环境的 IP 地址来源。
    ///
    /// - `0`：IpIp
    /// - `1`：独立服务器
    /// - `2`：基于 Linux ip 命令查询（仅限 linux 系统）
    ///
    /// 若未配置该项，则会使用 [`Configuration`] 中 `ip_source` 属性。
    ip_source: Option<IpSourceType>,
    /// 域名昵称，用于输出日志
    nickname: String,
    /// 域名 Cloudflare id
    id: String,
    /// 域名 Cloudflare zone id
    zone_id: String,
}

impl Domain {
    /// 获取绑定的本地 IP 地址
    pub fn bind_address(&self) -> Option<IpAddr> {
        self.bind_address
    }

    /// 获取刷新间隔，单位秒。
    pub fn fresh_interval(&self) -> Option<u64> {
        self.fresh_interval
    }

    /// 获取域名昵称，用于输出日志
    pub fn nickname(&self) -> &str {
        self.nickname.as_ref()
    }

    /// 获取域名 Cloudflare id
    pub fn id(&self) -> &str {
        self.id.as_ref()
    }

    /// 获取域名 Cloudflare zone id
    pub fn zone_id(&self) -> &str {
        self.zone_id.as_ref()
    }

    /// 获取出现错误时重试间隔，单位秒
    pub fn retry_interval(&self) -> Option<u64> {
        self.retry_interval
    }

    /// 获取 IP 来源方式
    pub fn ip_source(&self) -> Option<&IpSourceType> {
        self.ip_source.as_ref()
    }
}

/// Cloudflare 访问代理
// #[derive(serde::Deserialize, Debug, Clone)]
// pub struct Proxy {
//     url: String,
//     no_proxies: Vec<String>,
//     username: Option<String>,
//     password: Option<String>,
// }
#[derive(Debug, Clone)]
pub struct Proxy(reqwest::Proxy);

impl<'de> Deserialize<'de> for Proxy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ProxyVisitor;
        impl<'de> Visitor<'de> for ProxyVisitor {
            type Value = Proxy;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Cloudflare 访问代理配置")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut proxy_url = None;
                let mut basic_auth_username = None;
                let mut basic_auth_password = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "url" => proxy_url = Some(map.next_value::<String>()?),
                        "username" => basic_auth_username = Some(map.next_value::<String>()?),
                        "password" => basic_auth_password = Some(map.next_value::<String>()?),
                        _ => {}
                    }
                }

                let Some(proxy_url) = proxy_url else {
                    return Err(serde::de::Error::missing_field("proxy.url"));
                };
                let Ok(mut proxy) = reqwest::Proxy::https(proxy_url.as_str()) else {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(proxy_url.as_str()),
                        &"http, https or socks proxy url",
                    ));
                };

                match (basic_auth_username, basic_auth_password) {
                    (None, None) => {}
                    (None, Some(_)) => {
                        return Err(serde::de::Error::missing_field("proxy.username"))
                    }
                    (Some(_), None) => {
                        return Err(serde::de::Error::missing_field("proxy.password"))
                    }
                    (Some(username), Some(password)) => {
                        proxy = proxy.basic_auth(username.as_str(), password.as_str());
                    }
                }

                Ok(Proxy(proxy))
            }
        }

        deserializer.deserialize_map(ProxyVisitor)
    }
}

// #[derive(serde::Deserialize, Debug, Clone)]
// pub struct Log {
//     level: Option<log::LevelFilter>,
//     out: Option<PathBuf>,
//     err: Option<PathBuf>,
// }

// impl Log {
//     /// 获取日志级别
//     pub fn level(&self) -> Option<log::LevelFilter> {
//         self.level.clone()
//     }

//     /// 获取日志信息输出内容日志文件保存位置
//     pub fn out(&self) -> Option<&Path> {
//         self.out.as_ref().map(|path| path.as_path())
//     }

//     /// 获取日志错误输出内容日志文件保存位置
//     pub fn err(&self) -> Option<&Path> {
//         self.err.as_ref().map(|path| path.as_path())
//     }
// }

const DEFAULT_CONFIGURATION_NAME: &'static str = "config.json5";

/// 获取配置数据
pub fn configuration() -> Result<Configuration, Error> {
    let matches = args::arguments();
    match matches.value_of("config") {
        Some(value) => read_configuration(value),
        None => read_configuration(
            env::current_exe()
                .or(Err(Error::new_str("无法获取当前程序所在文件夹")))?
                .join(DEFAULT_CONFIGURATION_NAME),
        ),
    }
}

/// 从文件路径读取配置，并通过 `json5` 解析。
fn read_configuration<P>(path: P) -> Result<Configuration, Error>
where
    P: AsRef<Path>,
{
    let text =
        fs::read_to_string(path).or_else(|err| Err(Error::read_configuration_failure(err)))?;
    Ok(
        json5::from_str(text.as_str())
            .or_else(|err| Err(Error::read_configuration_failure(err)))?,
    )
}
