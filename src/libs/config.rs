use std::{env, fs, path::Path};

use reqwest::Url;
use serde::{
    de::{self, SeqAccess},
    Deserialize,
};

use super::{
    args,
    error::{StringifyError, StringifyResult},
    source::{IpIp, IpSource, Standalone},
    updater::Updater,
};

/// 默认刷新间隔
static DEFAULT_FRESH_INTERVAL_SECONDS: u64 = 15 * 60;
/// 默认全局出现错误时重试间隔
static DEFAULT_RETRY_INTERVAL_SECONDS: u64 = 5 * 60;

/// 可用的 IP 地址来源方式
///
/// - `0`：IpIp
/// - `1`：独立服务器
#[derive(Debug, Clone)]
pub enum IpSourceType {
    IpIp,
    Standalone(Url),
}

impl IpSourceType {
    fn to_ip_source(&self) -> Box<dyn IpSource> {
        match self {
            IpSourceType::IpIp => Box::new(IpIp),
            IpSourceType::Standalone(socket_addr) => Box::new(Standalone::new(socket_addr.clone())),
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
                formatter.write_str("可用的 IP 地址来源方式为：0(IpIp) 或 1(独立服务器)")
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    0 => Ok(IpSourceType::IpIp),
                    1 => Err(E::custom(
                        "IP 来源方式 2(独立服务器) 必须指定服务器访问地址",
                    )),
                    _ => Err(E::custom(format!("不支持的 IP 来源方式：{}", v))),
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let ip_source_type = match seq.next_element::<i64>()? {
                    Some(v) => v,
                    // 如果没有内容，则默认使用 IpIp
                    None => return Ok(IpSourceType::IpIp),
                };

                let socket_addr = match ip_source_type {
                    0 => return Ok(IpSourceType::IpIp),
                    1 => seq.next_element::<String>()?.unwrap_or(String::new()),
                    _ => {
                        return Err(de::Error::custom(format!(
                            "不支持的 IP 来源方式：{}",
                            ip_source_type
                        )))
                    }
                };

                let socket_addr = if socket_addr.is_empty() {
                    return Err(de::Error::custom(
                        "IP 来源方式 2(独立服务器) 必须指定服务器访问地址",
                    ));
                } else {
                    socket_addr
                };

                let socket_addr = if let Ok(socket_addr) = socket_addr.parse::<Url>() {
                    socket_addr
                } else {
                    return Err(de::Error::custom(format!("非法地址：{}", socket_addr)));
                };

                Ok(IpSourceType::Standalone(socket_addr))
            }
        }

        deserializer.deserialize_any(IpSourceTypeVisitor)
    }
}

/// 配置内容数据结构
///
/// 包含全局参数及需要刷新的域名列表。
#[derive(serde_derive::Deserialize, Debug)]
pub struct Configuration {
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
    ip_source: Option<IpSourceType>,
    /// Cloudflare 账号列表
    accounts: Vec<Account>,
}

impl Configuration {
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

    /// 通过当前配置内容创建 [`Updater`] 列表
    pub fn create_updaters(&self) -> Vec<Updater> {
        let mut updaters = vec![];
        self.accounts().iter().for_each(|account| {
            account.domains().iter().for_each(|domain| {
                let updater = Updater::new(
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
                );

                updaters.push(updater);
            })
        });

        updaters
    }

    /// 获取全局出现错误时重试间隔，单位秒。默认为 300 秒后。
    pub fn retry_interval(&self) -> u64 {
        self.retry_interval
            .unwrap_or(DEFAULT_RETRY_INTERVAL_SECONDS)
    }
}

/// Cloudflare 账号数据
#[derive(serde_derive::Deserialize, Debug)]
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
#[derive(serde_derive::Deserialize, Debug)]
pub struct Domain {
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

static DEFAULT_CONFIGURATION_NAME: &'static str = "config.json5";

/// 获取配置数据
pub fn configuration() -> StringifyResult<Configuration> {
    let matches = args::arguments();
    match matches.value_of("config") {
        Some(value) => read_configuration(value),
        None => read_configuration(env::current_dir().unwrap().join(DEFAULT_CONFIGURATION_NAME)),
    }
}

/// 从文件路径读取配置，并通过 `json5` 解析。
fn read_configuration<F>(file: F) -> StringifyResult<Configuration>
where
    F: AsRef<Path>,
{
    let text = fs::read_to_string(file)
        .or_else(|err| Err(StringifyError::read_configuration_failure(err)))?;
    Ok(json5::from_str(text.as_str())
        .or_else(|err| Err(StringifyError::read_configuration_failure(err)))?)
}
