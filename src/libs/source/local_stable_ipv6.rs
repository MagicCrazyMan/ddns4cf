use std::{fmt::Debug, net::IpAddr};

use async_trait::async_trait;

use crate::libs::error::Error;

use super::IpSource;

/// Linux 专用，使用
#[derive(Debug)]
pub struct LocalStableIPv6;

impl LocalStableIPv6 {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl IpSource for LocalStableIPv6 {
    async fn ip(&self, bind_address: Option<IpAddr>) -> Result<IpAddr, Error> {
        let response = reqwest::ClientBuilder::new()
            .local_address(bind_address)
            .build()?
            .get(self.0.as_ref())
            .send()
            .await
            .or_else(|err| {
                Err(Error::new(format!(
                    "访问独立服务器 {} 失败：{}",
                    self.0, err
                )))
            })?;

        let ip_addr = response
            .text()
            .await
            .ok()
            .and_then(|text| text.parse::<IpAddr>().ok())
            .ok_or(Error::new(format!(
                "从独立服务器 {} 中解析 IP 地址失败",
                self.0
            )))?;

        Ok(ip_addr)
    }

    fn name(&self) -> &'static str {
        "Standalone Server"
    }

    fn log(&self) -> String {
        self.0.to_string()
    }
}
