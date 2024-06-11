pub mod ipip;
#[cfg(feature = "linux")]
pub mod local_stable_ipv6;
pub mod standalone;

use std::{fmt::Debug, net::IpAddr};

use async_trait::async_trait;

use super::error::Error;

/// IP 地址来源
#[async_trait]
pub trait IpSource: Debug + Send + Sync {
    fn name(&self) -> &'static str;

    fn log(&self) -> String;

    /// 获取当前运行机器所处于的 ip 地址
    async fn ip(&self, bind_address: Option<IpAddr>) -> Result<IpAddr, Error>;
}
