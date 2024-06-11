use std::fmt::Display;

/// 字符串化错误，仅用于打印异常内容，不用作任何判断。
#[derive(Debug, Clone)]
pub struct Error(String);

impl Error {
    pub fn new<T>(reason: T) -> Self
    where
        T: AsRef<str>,
    {
        Self(reason.as_ref().to_string())
    }

    pub fn read_configuration_failure<E: std::error::Error>(err: E) -> Self {
        Self(format!("配置文件读取失败：{}", err))
    }

    pub fn cloudflare_network_failure<E: std::error::Error>(err: E) -> Self {
        Self(format!(
            "访问 Cloudflare 失败，请确认网络连接正常，错误原因：{}",
            err,
        ))
    }

    pub fn cloudflare_record_failure(reason: Option<String>) -> Self {
        Self(format!(
            "获取 Cloudflare DNS 记录详情失败，错误原因：{}",
            reason.unwrap_or("未知原因".to_string()),
        ))
    }

    pub fn cloudflare_update_failure(reason: Option<String>) -> Self {
        Self(format!(
            "更新 Cloudflare DNS 记录失败。错误原因：{}",
            reason.unwrap_or("未知原因".to_string())
        ))
    }

    pub fn cloudflare_deserialized_failure<E: std::error::Error>(err: E) -> Self {
        Self(format!("解析 Cloudflare 响应时出现错误，错误原因：{}", err,))
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self(format!("HTTP 请求出错：{value}"))
    }
}
