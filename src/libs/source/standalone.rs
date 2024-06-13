use std::{borrow::Cow, fmt::Debug, net::IpAddr, str::FromStr};

use async_trait::async_trait;
use reqwest::Url;

use crate::libs::error::Error;

use super::IpSource;

/// 从 独立服务器获取 IP 地址
#[derive(Debug)]
pub struct Standalone(Url);

impl Standalone {
    pub fn new(url: Url) -> Self {
        Self(url)
    }

    async fn send<T>(&self, bind_address: Option<IpAddr>) -> Result<T, Error>
    where
        T: FromStr,
    {
        let text = reqwest::ClientBuilder::new()
            .local_address(bind_address)
            .build()?
            .get(self.0.as_ref())
            .send()
            .await
            .or_else(|err| {
                Err(Error::new_string(format!(
                    "访问独立服务器 {} 失败：{}",
                    self.0, err
                )))
            })?
            .text()
            .await
            .or_else(|err| {
                Err(Error::new_string(format!(
                    "解析独立服务器 {} 消息失败：{}",
                    self.0, err
                )))
            })?;

        let ip_addr = text.parse::<T>().or_else(|_| {
            Err(Error::new_string(format!(
                "独立服务器 {} 响应消息并非合法 IP 地址",
                self.0
            )))
        })?;

        Ok(ip_addr)
    }
}

#[async_trait]
impl IpSource for Standalone {
    async fn ip(&self, bind_address: Option<IpAddr>) -> Result<IpAddr, Error> {
        self.send(bind_address).await
    }

    fn name(&self) -> &'static str {
        "Standalone Server"
    }

    fn info(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Owned(self.0.to_string()))
    }
}
