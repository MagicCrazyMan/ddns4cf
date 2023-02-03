use std::{fmt::Debug, net::IpAddr};

use async_trait::async_trait;
use reqwest::Url;

use super::error::{StringifyError, StringifyResult};

/// IP 地址来源
#[async_trait]
pub trait IpSource: Debug + Send + Sync {
    /// 获取当前运行机器所处于的 ip 地址
    async fn ip(&self, bind_address: Option<IpAddr>) -> StringifyResult<IpAddr>;
}

/// 从 IpIp 获取当前运行机器所处于的 ip 地址
#[derive(Debug)]
pub struct IpIp;

impl IpIp {
    pub fn new() -> Self {
        Self
    }
}

impl IpIp {
    async fn send_request(&self, bind_address: Option<IpAddr>) -> Result<String, reqwest::Error> {
        let client = reqwest::ClientBuilder::new().local_address(bind_address).user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36").build()?;
        let html = client
            .get("https://www.ipip.net/")
            .send()
            .await?
            .text()
            .await?;

        Ok(html)
    }
}

#[async_trait]
impl IpSource for IpIp {
    async fn ip(&self, bind_address: Option<IpAddr>) -> StringifyResult<IpAddr> {
        let text = self.send_request(bind_address).await.or_else(|err| {
            Err(StringifyError::new(format!(
                "获取 IpIp 网页时发生错误：{}",
                err
            )))
        })?;

        let html = scraper::Html::parse_document(text.as_str());
        let dom_selector = scraper::Selector::parse(".yourInfo a").unwrap();

        let ip = html
            .select(&dom_selector)
            .nth(0)
            .and_then(|tag| tag.text().next())
            .and_then(|raw_ip| raw_ip.parse::<IpAddr>().ok())
            .ok_or(StringifyError::new("解析 IpIp 网页失败"))?;

        Ok(ip)
    }
}

/// 从 独立服务器获取 IP 地址
#[derive(Debug)]
pub struct Standalone(Url);

impl Standalone {
    pub fn new(url: Url) -> Self {
        Self(url)
    }
}

#[async_trait]
impl IpSource for Standalone {
    async fn ip(&self, bind_address: Option<IpAddr>) -> StringifyResult<IpAddr> {
        let response = reqwest::ClientBuilder::new()
            .local_address(bind_address)
            .build()?
            .get(self.0.as_ref())
            .send()
            .await
            .or_else(|err| {
                Err(StringifyError::new(format!(
                    "访问独立服务器 {} 失败：{}",
                    self.0, err
                )))
            })?;

        let ip_addr = response
            .text()
            .await
            .ok()
            .and_then(|text| text.parse::<IpAddr>().ok())
            .ok_or(StringifyError::new(format!(
                "从独立服务器 {} 中解析 IP 地址失败",
                self.0
            )))?;

        Ok(ip_addr)
    }
}

#[cfg(test)]
mod tests {
    use crate::libs::error::StringifyResult;

    use super::{IpIp, IpSource};

    #[tokio::test]
    async fn test_ipip() -> StringifyResult<()> {
        let ip_source = IpIp;

        let ip = ip_source.ip(None).await?;
        println!("{}", ip);

        Ok(())
    }
}
