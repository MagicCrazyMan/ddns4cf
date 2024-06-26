use std::{borrow::Cow, fmt::Debug, net::IpAddr, sync::OnceLock};

use async_trait::async_trait;
use regex::Regex;

use crate::libs::error::Error;

use super::IpSource;

/// 从 IpIp 获取当前运行机器所处于的 ip 地址
#[derive(Debug)]
pub struct IpIp;

impl IpIp {
    pub fn new() -> Self {
        Self
    }
}

impl IpIp {
    async fn send(&self, bind_address: Option<IpAddr>) -> Result<String, reqwest::Error> {
        let client = reqwest::ClientBuilder::new().local_address(bind_address).user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36").build()?;
        let html = client
            .get("https://www.ipip.net/ip.html#")
            .send()
            .await?
            .text()
            .await?;

        Ok(html)
    }
}

#[async_trait]
impl IpSource for IpIp {
    async fn ip(&self, bind_address: Option<IpAddr>) -> Result<IpAddr, Error> {
        if let Some(bind_address) = bind_address.as_ref() {
            if bind_address.is_ipv6() {
                return Err(Error::new_str("IpIp 不支持获取 IPv6 地址"));
            }
        }

        let text = self.send(bind_address).await.or_else(|err| {
            Err(Error::new_string(format!(
                "获取 IpIp 网页时发生错误：{}",
                err
            )))
        })?;

        const IP_EXTRACT_REGEX: OnceLock<Regex> = OnceLock::new();
        let ip = IP_EXTRACT_REGEX
            .get_or_init(|| {
                Regex::new(
                    r"\$\('input\[name=ip\]'\).attr\('value', '(.+)'\).get\(0\).form.submit\(\);",
                )
                .unwrap()
            })
            .captures(text.as_str())
            .and_then(|captures| captures.get(1))
            .and_then(|matched| matched.as_str().parse::<IpAddr>().ok())
            .ok_or(Error::new_str("解析 IpIp 网页失败"))?;

        Ok(ip)
    }

    fn name(&self) -> &'static str {
        "IpIp"
    }

    fn info(&self) -> Option<Cow<'_, str>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::libs::error::Error;

    use super::{IpIp, IpSource};

    #[tokio::test]
    async fn test_ipip() -> Result<(), Error> {
        let ip_source = IpIp;

        let ip = ip_source.ip(None).await?;
        println!("{}", ip);

        Ok(())
    }
}
