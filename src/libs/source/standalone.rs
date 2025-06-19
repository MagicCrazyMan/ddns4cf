use std::{borrow::Cow, fmt::Debug, net::IpAddr, str::FromStr};

use async_trait::async_trait;
use reqwest::{Client, Url};

use crate::libs::error::Error;

use super::IpSource;

/// 从 独立服务器获取 IP 地址
#[derive(Debug)]
pub struct Standalone {
    url: Url,
    client: Client,
}

impl Standalone {
    pub fn new(url: Url, bind_address: Option<IpAddr>) -> Result<Self, reqwest::Error> {
        Ok(Self {
            url,
            client: reqwest::ClientBuilder::new()
                .local_address(bind_address)
                .build()?,
        })
    }

    async fn request<T>(&self) -> Result<T, Error>
    where
        T: FromStr,
    {
        let text = self
            .client
            .get(self.url.as_ref())
            .send()
            .await
            .or_else(|err| {
                Err(Error::new_string(format!(
                    "访问独立服务器 {} 失败：{}",
                    self.url, err
                )))
            })?
            .text()
            .await
            .or_else(|err| {
                Err(Error::new_string(format!(
                    "解析独立服务器 {} 消息失败：{}",
                    self.url, err
                )))
            })?;

        let ip_addr = text.parse::<T>().or_else(|_| {
            Err(Error::new_string(format!(
                "独立服务器 {} 响应消息并非合法 IP 地址",
                self.url
            )))
        })?;

        Ok(ip_addr)
    }
}

#[async_trait]
impl IpSource for Standalone {
    async fn ip(&self) -> Result<IpAddr, Error> {
        self.request().await
    }

    fn name(&self) -> &'static str {
        "Standalone Server"
    }

    fn info(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.url.as_str()))
    }
}
