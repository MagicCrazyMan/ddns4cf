#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod local_ipv6;
pub mod standalone;

use std::{borrow::Cow, fmt::Debug, net::IpAddr};

use async_trait::async_trait;

use super::error::Error;

/// IP 地址来源
#[async_trait]
pub trait IpSource: Debug + Send + Sync {
    /// 返回 IpSource 名称
    fn name(&self) -> &str;

    /// 返回用于日志输出的消息提示内容
    fn info(&self) -> Option<Cow<'_, str>>;

    /// 获取当前运行机器所处于的 IPv4 地址
    async fn ip(&self) -> Result<IpAddr, Error>;
}
