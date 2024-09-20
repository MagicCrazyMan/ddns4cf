use std::{sync::Arc, time::Duration};

use futures::future::join_all;
use log::{error, info};
use smallvec::SmallVec;
use tokio::{
    sync::{
        broadcast::{error::RecvError, Receiver, Sender},
        Mutex,
    },
    time::sleep,
};

use super::updater::Updater;

/// 自循环定时更新域名调度器
pub struct LoopingScheduler {
    updaters: SmallVec<[(Arc<Mutex<Updater>>, Receiver<()>); 4]>,
}

impl LoopingScheduler {
    /// 创建自循环定时更新域名调度器
    pub fn new<I>(updaters: I, termination_tx: &Sender<()>) -> Self
    where
        I: IntoIterator<Item = Arc<Mutex<Updater>>>,
    {
        let updaters = updaters
            .into_iter()
            .map(|updater| (updater, termination_tx.subscribe()))
            .collect::<SmallVec<[(Arc<Mutex<Updater>>, Receiver<()>); 4]>>();
        Self { updaters }
    }

    /// 启动自循环定时更新
    pub async fn start(self) {
        let mut handlers = Vec::with_capacity(self.updaters.len());

        // 启动循环更新器
        self.updaters
            .into_iter()
            .for_each(|(updater, mut termination_rx)| {
                let handler = tokio::spawn(async move {
                    loop {
                        let Ok(mut updater) = updater.try_lock() else {
                            continue;
                        };

                        let interval = match updater.update().await {
                            Ok(msg) => {
                                info!(
                                    "[{}] {}。{} 秒后进行下次检查。",
                                    updater.nickname, msg, updater.refresh_interval
                                );
                                updater.refresh_interval
                            }
                            Err(err) => {
                                error!(
                                    "[{}] {}。将在 {} 秒后重试",
                                    updater.nickname, err, updater.retry_interval
                                );
                                updater.retry_interval
                            }
                        };

                        drop(updater);

                        let abort = tokio::select! {
                            _ = termination_rx.recv() => true,
                            _ = sleep(Duration::from_secs(interval)) => false,
                        };
                        if abort {
                            break;
                        }
                    }
                });
                handlers.push(handler);
            });

        join_all(handlers).await;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyKind {
    OsSuspendResume,
}

/// 基于事件消息的域名更新调度器
pub struct NotifyScheduler {
    termination_rx: Receiver<()>,
    updaters: SmallVec<[Arc<Mutex<Updater>>; 4]>,
    notify_rx: Receiver<NotifyKind>,
}

impl NotifyScheduler {
    /// 创建事件消息域名调度器
    pub fn new(
        updaters: SmallVec<[Arc<Mutex<Updater>>; 4]>,
        notify_rx: Receiver<NotifyKind>,
        termination_tx: &Sender<()>,
    ) -> Self {
        Self {
            termination_rx: termination_tx.subscribe(),
            updaters,
            notify_rx,
        }
    }

    /// 启动消息监听更新
    pub async fn start(mut self) {
        loop {
            let abort = tokio::select! {
                _ = self.termination_rx.recv() => true,
                result = self.notify_rx.recv() => {
                    match result {
                        Ok(kind) => {
                            match kind {
                                NotifyKind::OsSuspendResume => info!("接收系统唤醒事件，触发域名刷新"),
                            };
                            false
                        },
                        Err(error) => match error {
                            RecvError::Closed => true,
                            RecvError::Lagged(_) => false
                        }
                    }
                },
            };
            if abort {
                break;
            }

            let handlers = self.updaters.iter().cloned().map(|updater| {
                tokio::spawn(async move {
                    let Ok(mut updater) = updater.try_lock() else {
                        return;
                    };

                    match updater.update().await {
                        Ok(msg) => {
                            info!("[{}] {}", updater.nickname, msg);
                        }
                        Err(err) => {
                            error!("[{}] {}", updater.nickname, err);
                        }
                    };
                    drop(updater);
                })
            });
            join_all(handlers).await;
        }
    }
}
