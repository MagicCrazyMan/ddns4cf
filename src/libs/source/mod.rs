pub mod ipip;
#[cfg(target_os = "linux")]
pub mod local_ipv6;
pub mod standalone;

use std::{borrow::Cow, fmt::Debug, net::IpAddr};

use async_trait::async_trait;

use super::error::Error;

/// IP 地址来源
#[async_trait]
pub trait IpSource: Debug + Send + Sync {
    fn name(&self) -> &'static str;

    fn info(&self) -> Option<Cow<'_, str>>;

    /// 获取当前运行机器所处于的 IPv4 地址
    async fn ip(&self, bind_address: Option<IpAddr>) -> Result<IpAddr, Error>;
}
