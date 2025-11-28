
#![cfg(target_os = "android")]


#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _: *mut std::os::raw::c_void) -> jni::sys::jint {

    // extern "C" {
    //     // __android_log_print in <android/log.h>
    //     // int __android_log_print(int prio, const char *tag, const char *fmt, ...);
    //     fn __android_log_print(prio: i32, tag: *const u8, fmt: *const u8, ...) -> i32;
    // }
    // use std::ffi::CString;
    // const ANDROID_LOG_INFO: i32 = 4;
    // let tag = CString::new("yuv").unwrap();
    // let msg = CString::new("JNI_OnLoad entered asdf").unwrap();
    // unsafe {
    //     __android_log_print(ANDROID_LOG_INFO, tag.as_ptr(), msg.as_ptr());
    // }
    

    // let _env = vm.get_env().expect("Cannot get reference to the JNIEnv");

    // // init_tracing();
    // let r = android_log::init_with(None, None);
    // if let Err(e) = r {
    //     log::error!("init log error {e:?}");
    //     return 0;
    // }

    // let r = crate::kit::async_rt::try_init();
    // if let Err(e) = r {
    //     log::error!("async_rt::try_init error {e:?}");
    //     return 0;
    // }

    jni::sys::JNI_VERSION_1_6
}

#[uniffi::export]
pub fn init_lib(
    log_dir: Option<String>,
    to_logcat: bool,
) -> String 
{
    use std::sync::OnceLock;
    static LOG_GUARD: OnceLock<android_log::LogGuard> = OnceLock::new();

    if LOG_GUARD.get().is_some() {
        let reason = format!("already init_lib");
        log::error!("{}", reason);
        return reason;
    };

    let r = android_log::init_with(
        log_dir.as_deref(), 
        to_logcat,
    );
    match r {
        Ok(guard) => {
            LOG_GUARD.get_or_init(||guard);
        },
        Err(e) => {
            return format!("init log error {e:?}");
        }
    }
    log::info!("init_lib: log_dir {:?}, to_logcat {:?}", log_dir, to_logcat);
    
    let r = crate::kit::async_rt::try_init();
    if let Err(e) = r {
        let reason = format!("async_rt::try_init error {e:?}");
        log::error!("{}", reason);
        return reason;
    }

    log::info!("init_lib ok");

    String::new()
}


mod android_log {
    use crate::kit::log::{LogFileArgs, make_file_layer};
    use anyhow::{Result, Context};
    use tracing_subscriber::layer::SubscriberExt;

    #[derive(Default)]
    pub struct LogGuard {
        _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
    }

    pub fn init_with(
        log_dir: Option<&str>, 
        to_logcat: bool,
    ) -> Result<LogGuard> {

        if log_dir.is_none() && !to_logcat {
            return Ok(LogGuard::default())
        }
        
        let crate_name = env!("CARGO_CRATE_NAME");

        let file_args = match log_dir {
            Some(dir) => {
                Some(LogFileArgs {
                    directory: dir.into(),
                    name_prefix: crate_name.into(),
                    rolling: tracing_appender::rolling::Rotation::NEVER,
                    ..Default::default()
                })
            },
            None => None,
        };
        let extra_mods: &[&str] = &[];
        let filter = crate::env_filter!(extra_mods)?;

        let timer = tracing_subscriber::fmt::time::ChronoLocal::new("%y-%m-%dT%H:%M:%S%.3f".into());

        let logcat_layer = if to_logcat {
            Some(make_logcat_layer(crate_name)?)
        } else {
            None
        };

        let (file_guard, file_layer) = match &file_args {
            Some(file_args) => {
                let (guard, layer) = make_file_layer(file_args, timer.clone())?;
                (Some(guard), Some(layer))
            },
            None => (None, None),
        };

        let atrace_layer = tracing_android_trace::AndroidTraceLayer::new();

        tracing::subscriber::set_global_default(
            tracing_subscriber::registry()
                .with(filter)
                .with(Some(atrace_layer))
                .with(logcat_layer)
                .with(file_layer)
        ).with_context(||"set_global_default failed")?;

        // 把 log crate 的日志转到 tracing
        tracing_log::LogTracer::init().with_context(||"failed to init LogTracer")?;
        log::set_max_level(log::LevelFilter::Debug);

        Ok(LogGuard { _guard: file_guard })
    }

    pub fn make_logcat_layer<S>(tag: &str) -> Result<impl tracing_subscriber::Layer<S>> 
    where 
        S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    {
        let tag = tracing_logcat::LogcatTag::Fixed(tag.to_string());
        let writer = tracing_logcat::LogcatMakeWriter::new(tag)
            .with_context(||"failed to init logcat writer")?;
        let layer = tracing_subscriber::fmt::layer()
            .without_time()
            .with_level(false)
            .with_writer(writer) 
            .with_ansi(false)
            .with_target(false);
        Ok(layer)
    }
}

