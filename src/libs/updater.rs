use std::{borrow::Cow, fmt::Display, net::IpAddr, time::Duration};

use bytes::Buf;
use log::{error, info};
use reqwest::{header, Client};
use tokio::time::sleep;

use super::{error::Error, source::IpSource};

/// Cloudflare API 响应
#[derive(serde::Deserialize, Debug)]
struct CloudflareResponse<T> {
    success: bool,
    errors: Option<Vec<CloudflareMessage>>,
    result: Option<T>,
}

/// Cloudflare API 消息
#[derive(serde::Deserialize, Debug)]
struct CloudflareMessage {
    code: u32,
    message: String,
}

impl Display for CloudflareMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Cloudflare 响应代码 {}：{}",
            self.code, self.message
        ))
    }
}

/// Cloudflare API 域名详情
#[derive(serde::Deserialize, Debug)]
struct CloudflareRecordDetails {
    r#type: String,
    name: String,
    content: IpAddr,
    ttl: usize,
    proxied: bool,
}

/// Cloudflare API 更新域名发送的消息负载
#[derive(serde::Serialize, Debug)]
struct CloudflareUpdateDNSBody<'a> {
    r#type: &'a str,
    ttl: usize,
    name: &'a str,
    content: &'a IpAddr,
    proxied: bool,
}

/// Cloudflare 域名更新器，所有更新相关的操作均由该结构负责完成。
#[derive(Debug)]
pub struct Updater {
    pub bind_address: Option<IpAddr>,
    pub refresh_interval: u64,
    pub retry_interval: u64,
    pub nickname: String,
    pub token: String,
    pub id: String,
    pub zone_id: String,
    cf_http_client: Client,
    ip_source: Box<dyn IpSource>,
    details: Option<CloudflareRecordDetails>,
}

impl Updater {
    /// 创建新更新器
    pub fn new(
        bind_address: Option<IpAddr>,
        ip_source: Box<dyn IpSource>,
        nickname: &str,
        token: &str,
        id: &str,
        zone_id: &str,
        refresh_interval: u64,
        retry_interval: u64,
        cf_http_client: Client,
    ) -> Self {
        Self {
            bind_address,
            ip_source,
            nickname: nickname.to_string(),
            token: token.to_string(),
            id: id.to_string(),
            zone_id: zone_id.to_string(),
            refresh_interval,
            retry_interval,
            cf_http_client,
            details: None,
        }
    }
}

impl Updater {
    /// 初始化
    pub async fn init(&mut self) {
        if let Some(bind_address) = self.bind_address {
            info!(
                "[{}] 正在使用手动绑定的本地地址：{}",
                self.nickname, bind_address
            );
        }

        info!(
            "[{}] 正在使用 IP 地址来源 {} {}",
            self.nickname,
            self.ip_source.name(),
            self.ip_source.info().unwrap_or(Cow::Borrowed(""))
        );

        info!("[{}] 初始化中...", self.nickname);
        self.prepare().await;
        info!("[{}] 初始化完毕", self.nickname);
    }

    /// 启动前预处理
    ///
    /// 将会访问 Cloudflare API 接口获取当前域名的详细信息
    async fn prepare(&mut self) {
        loop {
            match self.retrieve_dns_details().await {
                Ok(details) => {
                    self.details = Some(details);
                    break;
                }
                Err(err) => {
                    error!(
                        "[{}] {}。将在 {} 秒后重试",
                        self.nickname, err, self.retry_interval
                    );
                    sleep(Duration::from_secs(self.retry_interval)).await;
                }
            };
        }
    }

    /// 触发更新
    pub async fn update(&mut self) -> Result<String, Error> {
        let Some(old_details) = self.details.as_ref() else {
            return Err(Error::uninitialized());
        };

        let new_ip = self.ip_source.ip().await?;
        if new_ip == old_details.content {
            Ok(format!("IP 地址未发生变化，当前地址为：{}", new_ip))
        } else {
            info!("[{}] 成功获取最新 IP 地址：{}", self.nickname, new_ip);

            let new_details = self.update_dns_record(&new_ip).await?;

            let msg = format!(
                "Cloudflare DNS 记录更新成功，IP 地址更新为：{}（更新前为：{}）",
                new_details.content, old_details.content
            );
            self.details.replace(new_details);
            Ok(msg)
        }
    }

    /// 尝试获取 Cloudflare DNS 记录详情
    async fn retrieve_dns_details(&self) -> Result<CloudflareRecordDetails, Error> {
        // 访问 Cloudflare 获取当前 DNS 记录配置
        let bytes = self
            .cf_http_client
            .get(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
                self.zone_id, self.id
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .send()
            .await
            .or_else(|err| Err(Error::cloudflare_network_failure(err)))?
            .bytes()
            .await
            .or_else(|err| Err(Error::cloudflare_deserialized_failure(err)))?
            .reader();

        let details: CloudflareResponse<CloudflareRecordDetails> = simd_json::from_reader(bytes)
            .or_else(|err| Err(Error::cloudflare_deserialized_failure(err)))?;

        match (details.success, details.result) {
            (true, Some(details)) => Ok(details),
            (false, _) | (true, None) => {
                let message = details.errors.and_then(|errors| {
                    let message = errors
                        .into_iter()
                        .map(|error| error.to_string())
                        .collect::<Vec<_>>()
                        .join("；");
                    Some(Cow::Owned(message))
                });
                Err(Error::cloudflare_record_failure(message))
            }
        }
    }

    /// 更新 Cloudflare DNS 记录
    async fn update_dns_record(&self, new_ip: &IpAddr) -> Result<CloudflareRecordDetails, Error> {
        let Some(details) = self.details.as_ref() else {
            return Err(Error::uninitialized());
        };

        // 访问 Cloudflare 更新当前 DNS 记录配置
        let body = CloudflareUpdateDNSBody {
            r#type: &details.r#type,
            ttl: details.ttl,
            name: &details.name,
            content: new_ip,
            proxied: details.proxied,
        };

        let bytes = self
            .cf_http_client
            .put(format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
                self.zone_id, self.id
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            // 由于需要序列化，所以此处使用 body
            .body(simd_json::to_string::<CloudflareUpdateDNSBody>(&body).unwrap())
            .send()
            .await
            .or_else(|err| Err(Error::cloudflare_network_failure(err)))?
            .bytes()
            .await
            .or_else(|err| Err(Error::cloudflare_deserialized_failure(err)))?
            .reader();

        let details: CloudflareResponse<CloudflareRecordDetails> = simd_json::from_reader(bytes)
            .or_else(|err| Err(Error::cloudflare_deserialized_failure(err)))?;

        match (details.success, details.result) {
            (true, Some(details)) => Ok(details),
            (false, _) | (true, None) => {
                let message = details.errors.and_then(|errors| {
                    let message = errors
                        .into_iter()
                        .map(|error| error.to_string())
                        .collect::<Vec<_>>()
                        .join("；");
                    Some(Cow::Owned(message))
                });
                Err(Error::cloudflare_update_failure(message))
            }
        }
    }
}
