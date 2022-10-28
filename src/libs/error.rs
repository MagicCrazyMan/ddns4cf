use std::{error::Error, fmt::Display};

pub type StringifyResult<T> = std::result::Result<T, StringifyError>;

/// 字符串化错误，仅用于打印异常内容，不用作任何判断。
#[derive(Debug)]
pub struct StringifyError(String);

impl StringifyError {
    pub fn new<T: AsRef<str>>(reason: T) -> Self {
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

impl Display for StringifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for StringifyError {}
