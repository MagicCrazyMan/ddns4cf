use std::{borrow::Cow, fmt::Display};

/// 字符串化错误，仅用于打印异常内容，不用作任何判断。
#[derive(Debug, Clone)]
pub struct Error(Cow<'static, str>);

impl Error {
    pub fn new_str(reason: &'static str) -> Self {
        Self(Cow::Borrowed(&reason))
    }

    pub fn new_string(reason: String) -> Self {
        Self(Cow::Owned(reason))
    }

    pub fn uninitialized() -> Self {
        Self(Cow::Borrowed("Updater 未初始化"))
    }

    pub fn read_configuration_failure<E>(err: E) -> Self
    where
        E: std::error::Error,
    {
        Self::new_string(format!("配置文件读取失败：{}", err))
    }

    pub fn cloudflare_network_failure<E>(err: E) -> Self
    where
        E: std::error::Error,
    {
        Self::new_string(format!(
            "访问 Cloudflare 失败，请确认网络连接正常，错误原因：{}",
            err,
        ))
    }

    pub fn cloudflare_record_failure(reason: Option<Cow<'_, str>>) -> Self {
        match reason {
            Some(reason) => Self::new_string(format!(
                "获取 Cloudflare DNS 记录详情失败，错误原因：{}",
                reason,
            )),
            None => Self::new_str("获取 Cloudflare DNS 记录详情失败，错误原因：未知原因"),
        }
    }

    pub fn cloudflare_update_failure(reason: Option<Cow<'_, str>>) -> Self {
        match reason {
            Some(reason) => Self::new_string(format!(
                "更新 Cloudflare DNS 记录失败。错误原因：{}",
                reason,
            )),
            None => Self::new_str("更新 Cloudflare DNS 记录失败。错误原因：未知原因"),
        }
    }

    pub fn cloudflare_deserialized_failure<E>(err: E) -> Self
    where
        E: std::error::Error,
    {
        Self::new_string(format!("解析 Cloudflare 响应时出现错误，错误原因：{}", err))
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
        Self::new_string(format!("HTTP 请求出错：{value}"))
    }
}
