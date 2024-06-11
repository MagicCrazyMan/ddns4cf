use std::{
    borrow::Cow,
    fmt::Debug,
    net::{IpAddr, Ipv6Addr},
};

use async_trait::async_trait;
use serde::Deserialize;
use smallvec::SmallVec;
use tokio::process::Command;

use crate::libs::error::Error;

use super::IpSource;

/// Linux 专用，使用 `ip -6 -j addr` 命令获取 IPv6 地址来源。
/// 可以指定需要获取的网卡接口的名称，若未指定，则使用第一个符合匹配要求的 IPv6 地址。
///
/// 命令所输出的结果中，`addr_info`，将匹配以下规则
///
/// - `operstate` 为 `UP`
/// - `scope` 为 `global`
/// - `dynamic` 为 `true`
/// - `mngtmpaddr` 为 `true`
/// - `noprefixroute` 为 `true`
///
/// 将会使用首个匹配规则的地址
#[derive(Debug)]
pub struct LocalIPv6(Option<Cow<'static, str>>);

impl LocalIPv6 {
    pub fn new(interface_name: Option<Cow<'static, str>>) -> Self {
        Self(interface_name)
    }
}

#[async_trait]
impl IpSource for LocalIPv6 {
    async fn ip(&self, _: Option<IpAddr>) -> Result<IpAddr, Error> {
        let output = Command::new("ip")
            .arg("-6")
            .arg("-j")
            .arg("addr")
            .output()
            .await;

        let output = match output {
            Ok(output) => output,
            Err(err) => return Err(Error::new_string(format!("执行命令时发生错误：{err}"))),
        };

        let interfaces = match serde_json::from_slice::<SmallVec<[Interface; 8]>>(&output.stdout) {
            Ok(interfaces) => interfaces,
            Err(err) => return Err(Error::new_string(format!("解析 JSON 时发生错误：{err}"))),
        };

        let ip = interfaces
            .into_iter()
            .find(|interface| {
                let matched_name = match self.0.as_ref() {
                    Some(interface_name) => &interface.ifname == &*interface_name,
                    None => true,
                };
                matched_name && &interface.operstate == "UP"
            })
            .and_then(|interface| {
                interface
                    .addr_info
                    .into_iter()
                    .find(|info| &info.scope == "global" && !info.temporary && info.dynamic && info.mngtmpaddr && info.noprefixroute)
            })
            .map(|info| IpAddr::V6(info.local));

        ip.ok_or(Error::new_str("未匹配到合法的 IPv6 地址"))
    }

    fn name(&self) -> &'static str {
        "Local IPv6"
    }

    fn info(&self) -> Option<Cow<'_, str>> {
        match self.0.as_ref() {
            Some(interface_name) => Some(Cow::Owned(format!("指定网卡接口 {}", interface_name))),
            None => None,
        }
    }
}

#[derive(Deserialize)]
struct Interface<'a> {
    ifname: Cow<'a, str>,
    operstate: Cow<'a, str>,
    addr_info: Vec<AddrInfo<'a>>,
}

#[derive(Deserialize)]
struct AddrInfo<'a> {
    local: Ipv6Addr,
    scope: Cow<'a, str>,
    #[serde(default)]
    temporary: bool,
    #[serde(default)]
    dynamic: bool,
    #[serde(default)]
    mngtmpaddr: bool,
    #[serde(default)]
    noprefixroute: bool,
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::libs::{
        error::Error,
        source::{local_ipv6::LocalIPv6, IpSource},
    };

    #[tokio::test]
    async fn test_local_ipv6() -> Result<(), Error> {
        let ip_source = LocalIPv6::new(Some(Cow::Borrowed("enp2s0")));

        let ip = ip_source.ip(None).await?;
        println!("{}", ip);

        Ok(())
    }
}
