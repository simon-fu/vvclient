

use std::{collections::HashMap, time::Duration};

use crate::{kit::astr::AStr, proto};



#[derive(Debug, Clone)]
pub struct JoinConfig {
    pub user_id: AStr,

    pub room_id: AStr,

    pub advance: JoinAdvanceArgs,
}

#[derive(Debug, Clone, Default)]
pub struct JoinAdvanceArgs {
    pub user_ext: Option<AStr>,

    pub user_tree: Option<Vec<proto::UpdateTreeRequest>>, 

    pub connection: ConnectionConfig,

    pub batch: Option<bool>,

    pub token: Option<AStr>,

    pub client_info: Option<ClientInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub platform: Option<AStr>,
    pub sdk_name: Option<AStr>,
    pub sdk_version: Option<AStr>,
    pub device: Option<HashMap<AStr, AStr>>,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionConfig {
    /// 单次连接尝试的超时时间（每次 dial/syn 的超时）。
    /// `None` 表示使用默认值超时。
    pub connect_timeout: Option<Duration>,

    /// 所有重试的总共允许时长（从第一个尝试开始算起的截止时间）。
    /// `None` 表示使用默认值超时。
    pub max_timeout: Option<Duration>,

    /// 每次重试前等待的基础间隔（fixed interval）。
    /// `None` 表示使用默认值
    pub retry_interval: Option<Duration>,

    /// 忽略服务器的证书  
    pub ignore_server_cert: bool,

    /// 是否启用业务心跳
    pub heartbeat_enable: Option<bool>,

    /// 心跳发送间隔
    pub heartbeat_interval: Option<Duration>,

    /// 心跳超时阈值
    pub heartbeat_timeout: Option<Duration>,

    // /// 禁止 nagle 算法
    // pub disable_nagle: bool,
}

impl ConnectionConfig {
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout.unwrap_or(Duration::from_secs(5))
    }

    pub fn max_timeout(&self) -> Duration {
        self.max_timeout.unwrap_or(Duration::from_secs(10))
    }

    pub fn retry_interval(&self) -> Duration {
        self.retry_interval.unwrap_or(Duration::from_secs(1))
    }

    pub fn heartbeat_enable(&self) -> bool {
        self.heartbeat_enable.unwrap_or(true)
    }

    pub fn heartbeat_interval(&self) -> Duration {
        self.heartbeat_interval.unwrap_or(Duration::from_secs(10))
    }

    pub fn heartbeat_timeout(&self) -> Duration {
        self.heartbeat_timeout.unwrap_or(Duration::from_secs(30))
    }
}
