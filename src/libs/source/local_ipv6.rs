use std::{
    borrow::Cow,
    net::{IpAddr, Ipv6Addr},
};

use async_trait::async_trait;

use crate::libs::error::Error;

use super::IpSource;

/// Linux 和 Windows 专用，使用本机命令获取 IPv6 地址。
/// 可以指定需要获取的网卡接口的名称，若未指定，则使用第一个符合匹配要求的 IPv6 地址。
///
/// - 针对 Linux 系统
///
/// 使用 `ip -6 -j addr` 命令，对于所输出的结果中匹配以下规则：
///
/// - `operstate` 为 `UP`
/// - `scope` 为 `global`
/// - `dynamic` 为 `true`
/// - `mngtmpaddr` 为 `true`
/// - `noprefixroute` 为 `true`
///
/// 将会使用首个匹配规则的地址
///
/// - 针对 Windows 系统
///
/// 使用基于 Powershell 的命令 `Get-NetIPAddress -AddressFamily IPv6 -PolicyStore ActiveStore [-InterfaceAlias <interface_name>] | ConvertTo-JSON`。
///
/// 将会使用首个非本地、非回环地址、非多播、非未指定的地址
#[derive(Debug)]
pub struct LocalIPv6(Option<Cow<'static, str>>);

impl LocalIPv6 {
    pub fn new(interface_name: Option<Cow<'static, str>>) -> Self {
        Self(interface_name)
    }

    #[cfg(target_os = "linux")]
    async fn ip_linux(&self) -> Result<IpAddr, Error> {
        use serde::Deserialize;
        use smallvec::SmallVec;
        use tokio::process::Command;

        #[derive(Deserialize)]
        struct Interface<'a> {
            ifname: &'a str,
            operstate: &'a str,
            addr_info: Vec<AddrInfo<'a>>,
        }

        #[derive(Deserialize)]
        struct AddrInfo<'a> {
            local: Ipv6Addr,
            scope: &'a str,
            #[serde(default)]
            temporary: bool,
            #[serde(default)]
            dynamic: bool,
            #[serde(default)]
            mngtmpaddr: bool,
            #[serde(default)]
            noprefixroute: bool,
        }

        let output = Command::new("ip")
            .arg("-6")
            .arg("-j")
            .arg("addr")
            .output()
            .await;

        let mut output = match output {
            Ok(output) => output,
            Err(err) => return Err(Error::new_string(format!("执行命令时发生错误：{err}"))),
        };

        let interfaces = match simd_json::from_slice::<SmallVec<[Interface; 8]>>(&mut output.stdout)
        {
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
                matched_name && interface.operstate == "UP"
            })
            .and_then(|interface| {
                interface.addr_info.into_iter().find(|info| {
                    info.scope == "global"
                        && !info.temporary
                        && info.dynamic
                        && info.mngtmpaddr
                        && info.noprefixroute
                })
            })
            .map(|info| IpAddr::V6(info.local));

        ip.ok_or(Error::new_str("未匹配到合法的 IPv6 地址"))
    }

    #[cfg(target_os = "windows")]
    async fn ip_windows(&self) -> Result<IpAddr, Error> {
        use std::str::FromStr;

        use serde::{Deserialize, Serialize};
        use tokio::process::Command;

        #[derive(Serialize, Deserialize)]
        struct NetIPAddress<'a> {
            #[serde(rename = "IPAddress")]
            ip_address: &'a str,
        }

        const EMPTY_LIST: Vec<NetIPAddress> = Vec::new();

        let mut command = Command::new("powershell");
        command
            .arg("-Command")
            .arg("$OutputEncoding")
            .arg("=")
            .arg("[System.Console]::OutputEncoding")
            .arg("=")
            .arg("[System.Console]::InputEncoding")
            .arg("=")
            .arg("[System.Text.Encoding]::Unicode;")
            .arg("Get-NetIPAddress")
            .arg("-AddressFamily")
            .arg("IPv6")
            .arg("-PolicyStore")
            .arg("ActiveStore");
        if let Some(interface_name) = self.0.as_ref() {
            command.arg("-InterfaceAlias").arg(interface_name.as_ref());
        };
        command.arg("| ConvertTo-JSON");

        let output = command.output().await;
        let output = match output {
            Ok(output) => output,
            Err(err) => return Err(Error::new_string(format!("执行命令时发生错误：{err}"))),
        };
        let mut output = String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(
                output.stdout.as_ptr() as *const u16,
                output.stdout.len() / 2,
            )
        });

        let addresses = unsafe {
            simd_json::from_str::<Vec<NetIPAddress>>(output.as_mut_str()).unwrap_or(EMPTY_LIST)
        };

        let address = addresses
            .into_iter()
            .filter_map(|NetIPAddress { ip_address }| Ipv6Addr::from_str(&ip_address).ok())
            .filter(|address| {
                !address.is_loopback()
                    && !address.is_unspecified()
                    && !address.is_multicast()
                    && !address.is_unicast_link_local()
                    && !address.is_unique_local()
            })
            .next()
            .map(|address| IpAddr::V6(address));

        address.ok_or(Error::new_str("未匹配到合法的 IPv6 地址"))
    }
}

#[async_trait]
impl IpSource for LocalIPv6 {
    async fn ip(&self) -> Result<IpAddr, Error> {
        #[cfg(target_os = "linux")]
        {
            return self.ip_linux().await;
        }
        #[cfg(target_os = "windows")]
        {
            return self.ip_windows().await;
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            unimplemented!()
        }
    }

    fn name(&self) -> &str {
        "Local IPv6"
    }

    fn info(&self) -> Option<Cow<'_, str>> {
        match self.0.as_ref() {
            Some(interface_name) => Some(Cow::Owned(format!("指定网卡接口 {}", interface_name))),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::libs::{
        error::Error,
        source::{local_ipv6::LocalIPv6, IpSource},
    };

    #[tokio::test]
    async fn test_local_ipv6() -> Result<(), Error> {
        let ip_source = LocalIPv6::new(None);

        let ip = ip_source.ip().await?;
        println!("{}", ip);

        Ok(())
    }
}
