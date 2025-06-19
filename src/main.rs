use std::sync::{atomic::AtomicPtr, Arc};

use futures::future::join_all;
use libs::{
    config,
    error::Error,
    scheduler::{LoopingScheduler, NotifyKind, NotifyScheduler},
    updater::Updater,
};
use log::{error, info};
use smallvec::SmallVec;
use tokio::{
    signal,
    sync::{
        broadcast::{self, error::SendError, Sender},
        Mutex,
    },
};
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, ERROR_UNHANDLED_EXCEPTION, HANDLE},
    System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
    },
    UI::WindowsAndMessaging::{DEVICE_NOTIFY_CALLBACK, PBT_APMRESUMEAUTOMATIC},
};

mod libs;

fn main() {
    setup_logger();
    match start() {
        Ok(_) => {}
        Err(err) => error!("{}", err),
    }
}

fn setup_logger() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{:5}]{}",
                chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(if cfg!(test) {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .level_for(env!("CARGO_PKG_NAME"), log::LevelFilter::Info)
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

struct OsSuspendResumeUnregister {
    tx_ptr: AtomicPtr<Sender<NotifyKind>>,
    #[cfg(target_os = "windows")]
    handle: HPOWERNOTIFY,
}

impl OsSuspendResumeUnregister {
    fn unregister(self) {
        #[cfg(target_os = "windows")]
        unsafe {
            let error = PowerUnregisterSuspendResumeNotification(self.handle);
            if error.is_err() {
                log::warn!(
                    "注销 Windows Suspend Resume 通知事件失败。错误代码: {}",
                    error.to_hresult()
                );
            } else {
                info!("注销 Windows Suspend Resume 通知事件成功");
            }
        }

        unsafe {
            drop(Box::from_raw(self.tx_ptr.into_inner()));
        }
    }
}

/// 注册系统挂起后恢复事件。当前仅支持 Windows 系统，其他系统不会接收到消息。
fn listen_os_suspend_resume() -> Option<(Sender<NotifyKind>, OsSuspendResumeUnregister)> {
    #[cfg(target_os = "windows")]
    unsafe {
        let (tx, _) = broadcast::channel(1);
        let tx_ptr = AtomicPtr::new(Box::leak(Box::new(tx.clone())));

        unsafe extern "system" fn callback(
            context: *const core::ffi::c_void,
            r#type: u32,
            _: *const core::ffi::c_void,
        ) -> u32 {
            let tx = &*(context as *mut Sender<NotifyKind>);
            match r#type {
                PBT_APMRESUMEAUTOMATIC => match tx.send(NotifyKind::OsSuspendResume) {
                    Ok(_) => ERROR_SUCCESS.0,
                    Err(_) => ERROR_UNHANDLED_EXCEPTION.0,
                },
                _ => ERROR_SUCCESS.0,
            }
        }
        let mut recipient = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(callback),
            Context: *tx_ptr.as_ptr() as *mut _ as *mut core::ffi::c_void,
        };

        let mut handle = std::ptr::null_mut();
        let error = PowerRegisterSuspendResumeNotification(
            DEVICE_NOTIFY_CALLBACK,
            HANDLE(&mut recipient as *mut _ as *mut core::ffi::c_void),
            &mut handle,
        );
        match error {
            ERROR_SUCCESS => {
                info!("注册 Windows Suspend Resume 通知事件成功");
                let unregister = OsSuspendResumeUnregister {
                    tx_ptr,
                    handle: HPOWERNOTIFY(handle as isize),
                };

                return Some((tx, unregister));
            }
            _ => {
                log::warn!(
                    "注册 Windows Suspend Resume 通知事件失败。错误代码: {}",
                    error.to_hresult()
                );
                return None;
            }
        }
    }

    #[allow(unreachable_code)]
    None
}

fn send_terminate(termination_tx: Sender<()>) -> Result<(), SendError<()>> {
    termination_tx.send(())?;
    info!("正在停止所有 Schedulers...");
    Ok(())
}

fn listen_ctrl_c(termination_tx: Sender<()>) {
    tokio::spawn(async move {
        signal::ctrl_c().await.unwrap();
        send_terminate(termination_tx).unwrap();
    });
}

fn listen_signal(termination_tx: Sender<()>) {
    #[cfg(target_os = "linux")]
    tokio::spawn(async move {
        let mut stream =
            signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        stream.recv().await;
        send_terminate(termination_tx).unwrap();
    });
}

async fn init_updaters(updaters: &[Arc<Mutex<Updater>>]) {
    join_all(updaters.iter().map(|updater| async move {
        updater.lock().await.init().await;
    }))
    .await;
}

async fn start_schedulers(
    updaters: SmallVec<[Arc<Mutex<Updater>>; 4]>,
    termination_tx: Sender<()>,
) {
    let mut handlers = Vec::new();

    // 启动循环更新器
    {
        let scheduler = LoopingScheduler::new(updaters.clone(), &termination_tx);
        handlers.push(tokio::spawn(async move {
            scheduler.start().await;
        }));
    }

    // 启动系统挂起恢复事件监听
    if let Some((notify_tx, unregister)) = listen_os_suspend_resume() {
        let scheduler =
            NotifyScheduler::new(updaters.clone(), notify_tx.subscribe(), &termination_tx);
        let handler = tokio::spawn(async move {
            scheduler.start().await;
            unregister.unregister();
        });
        handlers.push(handler);
    }

    join_all(handlers).await;
}

fn start() -> Result<(), Error> {
    info!("启动 ddns4cf，版本: {}", env!("CARGO_PKG_VERSION"));
    info!("程序运行 pid：{}", std::process::id());

    let updaters = config::configuration()?.create_updaters()?;

    if updaters.len() == 0 {
        info!("未设置需要更新的域名信息，ddns4cf 已中止");
    } else {
        let updater_len = updaters.len();

        let main = async move {
            let (termination_tx, mut termination_rx) = broadcast::channel::<()>(1);
            listen_ctrl_c(termination_tx.clone());
            listen_signal(termination_tx.clone());

            // 初始化
            tokio::select! {
                _ = init_updaters(&updaters) => {}
                _ = termination_rx.recv() => return,
            }

            // 启动调度器
            start_schedulers(updaters, termination_tx).await;
        };

        if updater_len == 1 {
            info!("正在使用单线程模式运行");

            // 如果只有一个 Updater，使用单线程运行时
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(main);
        } else {
            info!("正在使用多线程模式运行");

            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(main);
        }
    }

    Ok(())
}
