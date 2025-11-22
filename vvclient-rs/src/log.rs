// 以下rust代码为什么只打印出来 "tracing info"， 而没有打印 "log info"

use std::borrow::Cow;

use anyhow::{Context as _, Result};
use tracing_subscriber::{fmt::{Layer, time::FormatTime}, layer::SubscriberExt};


// pub fn init_log_stdout() -> Result<()> {

//     let timer = tracing_subscriber::fmt::time::ChronoLocal::new("%y-%m-%dT%H:%M:%S%.3f".into());

//     let crate_name = env!("CARGO_CRATE_NAME");
//     let pkg_name = env!("CARGO_PKG_NAME");
//     let extra_mods: &[&str] = &[];

//     // let filter = make_env_filter(crate_name, pkg_name, &[])?;

    
//     let filter: tracing_subscriber::EnvFilter = if let Ok(v) = std::env::var(tracing_subscriber::EnvFilter::DEFAULT_ENV) {
//         v.into()
//     } else {
//         // let crate_name = env!("CARGO_CRATE_NAME");
//         // let packet_name = env!("CARGO_PKG_NAME");
//         let underline_crate_name = crate_name.replace("-", "_");
//         let underline_pkg_name = pkg_name.replace("-", "_");
//         let mut expr = format!("{underline_crate_name}=debug");
//         if underline_crate_name != underline_pkg_name {
//             expr.push_str(",");
//             expr.push_str(&format!("{underline_pkg_name}=debug"));
//         }

//         for extra in extra_mods {
//             expr.push_str(",");
//             expr.push_str(&format!("{extra}=debug"));
//         }
        
//         expr.into()
//     };

//     let stdout_layer = Layer::new()
//         .with_ansi(true)
//         .with_timer(timer)
//         .with_target(false)
//         .with_writer(std::io::stdout);

//     // let stdout_layer = Some(stdout_layer);

//     tracing::subscriber::set_global_default(
//         tracing_subscriber::registry()
//             .with(filter)
//             .with(stdout_layer)
//     ).with_context(||"set_global_default failed")?;
    
//     // 把 log crate 的日志转到 tracing
//     tracing_log::LogTracer::init().with_context(||"failed to init LogTracer")?;
//     log::set_max_level(log::LevelFilter::Debug);

//     tracing::debug!("tracing info");
//     log::debug!("log info");
    
//     Ok(())
// }

// // 以下rust代码为什么都能打印出来 "tracing info" 和 "log info"
// pub fn init_log_stdout2() -> Result<()> {

//     let timer = tracing_subscriber::fmt::time::ChronoLocal::new("%y-%m-%dT%H:%M:%S%.3f".into());

//     let crate_name = env!("CARGO_CRATE_NAME");
//     let pkg_name = env!("CARGO_PKG_NAME");
//     let extra_mods: &[&str] = &[];

//     // let filter = make_env_filter(crate_name, pkg_name, &[])?;

    
//     let filter: tracing_subscriber::EnvFilter = if let Ok(v) = std::env::var(tracing_subscriber::EnvFilter::DEFAULT_ENV) {
//         v.into()
//     } else {
//         // let crate_name = env!("CARGO_CRATE_NAME");
//         // let packet_name = env!("CARGO_PKG_NAME");
//         let underline_crate_name = crate_name.replace("-", "_");
//         let underline_pkg_name = pkg_name.replace("-", "_");
//         let mut expr = format!("{underline_crate_name}=debug");
//         if underline_crate_name != underline_pkg_name {
//             expr.push_str(",");
//             expr.push_str(&format!("{underline_pkg_name}=debug"));
//         }

//         for extra in extra_mods {
//             expr.push_str(",");
//             expr.push_str(&format!("{extra}=debug"));
//         }
        
//         expr.into()
//     };

//     tracing_subscriber::fmt()
//         .with_max_level(tracing::metadata::LevelFilter::DEBUG)
//         .with_env_filter(filter)
//         .with_timer(timer)
//         .with_target(false)
//         .with_ansi(true)
//         .init();
    
    
//     tracing::info!("tracing info");
//     log::info!("log info");
    
//     Ok(())
// }

#[derive(Debug)]
pub struct LogFileArgs {
    pub rolling: tracing_appender::rolling::Rotation,
    pub directory: String,
    pub name_prefix: String,
    pub name_suffix: Option<String>,
}

impl Default for LogFileArgs {
    fn default() -> Self {
        Self { 
            rolling: tracing_appender::rolling::Rotation::DAILY, 
            directory: Default::default(), 
            name_prefix: Default::default(),
            name_suffix: None,
        }
    }
}

#[derive(Default)]
pub struct LogGuard {
    _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}


#[macro_export]
macro_rules! init_log {
    () => {
        $crate::log::init_log_with(
            $crate::env_filter!()?, 
            None,
            None,
        )
    };

    ($extra_mods:expr) => {
        $crate::log::init_log_with(
            $crate::env_filter!($extra_mods)?,
            None,
            None,
        )
    };

    ($extra_mods:expr, $file_args:expr) => {
        $crate::log::init_log_with(
            $crate::env_filter!($extra_mods)?,
            $file_args,
            None,
        )
    };

    ($extra_mods:expr, $file_args:expr, $to_std_out:expr) => {
        $crate::log::init_log_with(
            $crate::env_filter!($extra_mods)?,
            $file_args,
            $to_std_out,
        )
    };
}

// pub fn init_log_with(crate_name: &str, packet_name: &str, extra_mods: &[&str], file_args: Option<&LogFileArgs>) -> Result<LogGuard> {


//     // println!("CARGO_PKG_NAME: {}", env!("CARGO_PKG_NAME"));
//     // println!("CARGO_CRATE_NAME: {}", env!("CARGO_CRATE_NAME"));


//     // https://time-rs.github.io/book/api/format-description.html

//     // // let fmts = time::macros::format_description!("[hour]:[minute]:[second].[subsecond digits:3]");
//     // let fmts = time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]");

//     // // let offset = time::UtcOffset::current_local_offset().expect("should get local offset!");
//     // // let timer = tracing_subscriber::fmt::time::OffsetTime::new(offset, fmts);

//     // let timer = tracing_subscriber::fmt::time::LocalTime::new(fmts); 

//     let timer = tracing_subscriber::fmt::time::ChronoLocal::new("%y-%m-%dT%H:%M:%S%.3f".into());
    
//     // let filter = if cfg!(debug_assertions) {
//     //     if let Ok(v) = std::env::var(tracing_subscriber::EnvFilter::DEFAULT_ENV) {
//     //         v.into()
//     //     } else {
//     //         // let crate_name = env!("CARGO_CRATE_NAME");
//     //         // let packet_name = env!("CARGO_PKG_NAME");
//     //         let underline_crate_name = crate_name.replace("-", "_");
//     //         let underline_packet_name = packet_name.replace("-", "_");
//     //         let mut expr = format!("{underline_crate_name}=debug");
//     //         if packet_name != crate_name {
//     //             expr.push_str(",");
//     //             expr.push_str(&format!("{underline_packet_name}=debug"));
//     //         }

//     //         for extra in extra_mods {
//     //             expr.push_str(",");
//     //             expr.push_str(&format!("{extra}=debug"));
//     //         }
            
//     //         expr.into()
//     //     }
//     // } else {
//     //     tracing_subscriber::EnvFilter::builder()
//     //     .with_default_directive(tracing::level_filters::LevelFilter::DEBUG.into())
//     //     .from_env_lossy()
//     // };

//     let filter: tracing_subscriber::EnvFilter = if let Ok(v) = std::env::var(tracing_subscriber::EnvFilter::DEFAULT_ENV) {
//         v.into()
//     } else {
//         // let crate_name = env!("CARGO_CRATE_NAME");
//         // let packet_name = env!("CARGO_PKG_NAME");
//         let underline_crate_name = crate_name.replace("-", "_");
//         let underline_packet_name = packet_name.replace("-", "_");
//         let mut expr = format!("{underline_crate_name}=debug");
//         if underline_crate_name != underline_packet_name {
//             expr.push_str(",");
//             expr.push_str(&format!("{underline_packet_name}=debug"));
//         }

//         for extra in extra_mods {
//             expr.push_str(",");
//             expr.push_str(&format!("{extra}=debug"));
//         }
        
//         expr.into()
//     };

//     let ansi = if file_args.is_some() {
//         false
//     } else {
//         if cfg!(debug_assertions) {
//             true
//         } else {
//             false
//         }
//     };

//     let builder =     tracing_subscriber::fmt()
//         .with_max_level(tracing::metadata::LevelFilter::DEBUG)
//         .with_env_filter(filter)
//         // .with_env_filter("rtun=debug,rserver=debug")
//         // .with_writer(w)
//         .with_timer(timer)
//         .with_target(false)
//         .with_ansi(ansi);
//         // .with_ansi(!file_args.is_some());

//     if let Some(file_args) = file_args {
//         let log_path = std::path::Path::new(&file_args.directory);

//         if !log_path.exists() {
//             std::fs::create_dir_all(log_path)
//                 .with_context(||format!("failed to make log dir [{}]", file_args.directory))?;
//         } else {
//             if !log_path.is_dir() {
//                 anyhow::bail!("log path must be directory [{}]", file_args.directory)
//             }
//         }

//         let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
//             .filename_prefix(&file_args.name_prefix)
//             .filename_suffix("log")
//             .rotation(file_args.rolling.clone())
//             .max_log_files(30)
//             .build(&file_args.directory)
//             .with_context(||"build log file appender faile")?;

//         let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

//         builder
//             .with_writer(non_blocking)
//             .init();
//         Ok(LogGuard {
//             _guard: Some(guard),
//         })
//     } else {
//         builder.init();
//         Ok(LogGuard {
//             _guard: None,
//         })
//     }

// }

pub fn init_log_with(
    filter: tracing_subscriber::EnvFilter, 
    file_args: Option<&LogFileArgs>, 
    to_std_out: Option<bool>,
) -> Result<LogGuard> {
    let to_stdout = to_std_out.unwrap_or(file_args.is_none());

    let timer = tracing_subscriber::fmt::time::ChronoLocal::new("%y-%m-%dT%H:%M:%S%.3f".into());

    let stdout_layer = if to_stdout {
        Some(make_stdout_layer(timer.clone())?)
    } else {
        None
    };

    let (file_guard, file_layer) = match file_args {
        Some(file_args) => {
            let (guard, layer) = make_file_layer(file_args, timer.clone())?;
            (Some(guard), Some(layer))
        },
        None => (None, None),
    };

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(file_layer)
    ).with_context(||"set_global_default failed")?;

    // 把 log crate 的日志转到 tracing
    tracing_log::LogTracer::init().with_context(||"failed to init LogTracer")?;
    log::set_max_level(log::LevelFilter::Debug);

    Ok(LogGuard { _guard: file_guard })
}

// pub fn init_log_layer2<L1, L2, S>(
//     filter: tracing_subscriber::EnvFilter, 
//     layer1: Option<L1>, 
//     layer2: Option<L2>, 
// ) -> Result<()> 
// where 
//     L1: tracing_subscriber::Layer<S>,
//     L2: tracing_subscriber::Layer<S>,
//     S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
// {
//     let timer = tracing_subscriber::fmt::time::ChronoLocal::new("%y-%m-%dT%H:%M:%S%.3f".into());


//     tracing::subscriber::set_global_default(
//         tracing_subscriber::registry()
//             .with(filter)
//             .with(layer1)
//             .with(layer2)
//     ).with_context(||"set_global_default failed")?;

//     Ok(())
// }

#[macro_export]
macro_rules! env_filter {
    () => {
        $crate::log::make_env_filter(env!("CARGO_CRATE_NAME"), env!("CARGO_PKG_NAME"), &[])
    };

    ($extra_mods:expr) => {
        $crate::log::make_env_filter(env!("CARGO_CRATE_NAME"), env!("CARGO_PKG_NAME"), $extra_mods)
    };
}

pub fn make_env_filter(crate_name: &str, packet_name: &str, extra_mods: &[&str]) -> Result<tracing_subscriber::EnvFilter> {

    let filter: tracing_subscriber::EnvFilter = if let Ok(v) = std::env::var(tracing_subscriber::EnvFilter::DEFAULT_ENV) {
        v.into()
    } else {
        // let crate_name = env!("CARGO_CRATE_NAME");
        // let packet_name = env!("CARGO_PKG_NAME");
        let underline_crate_name = crate_name.replace("-", "_");
        let underline_packet_name = packet_name.replace("-", "_");
        let mut expr = format!("{underline_crate_name}=debug");
        if underline_crate_name != underline_packet_name {
            expr.push_str(",");
            expr.push_str(&format!("{underline_packet_name}=debug"));
        }

        for extra in extra_mods {
            expr.push_str(",");
            expr.push_str(&format!("{extra}=debug"));
        }
        
        expr.into()
    };


    Ok(filter)
}

pub fn make_stdout_layer<T, S>(timer: T) -> Result<impl tracing_subscriber::Layer<S>> 
where 
    T: FormatTime + 'static,
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let layer = Layer::new()
        .with_ansi(true)
        .with_timer(timer)
        .with_target(false)
        .with_writer(std::io::stdout);
    Ok(layer)
}

pub fn make_file_layer<T, S>(file_args: &LogFileArgs, timer: T) -> Result<(tracing_appender::non_blocking::WorkerGuard, impl tracing_subscriber::Layer<S>)> 
where 
    T: FormatTime + 'static,
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    let log_path = std::path::Path::new(&file_args.directory);

    if !log_path.exists() {
        std::fs::create_dir_all(log_path)
            .with_context(||format!("failed to make log dir [{}]", file_args.directory))?;
    } else {
        if !log_path.is_dir() {
            anyhow::bail!("log path must be directory [{}]", file_args.directory)
        }
    }

    let filename_prefix: Cow<str> = if file_args.rolling == tracing_appender::rolling::Rotation::NEVER {
        const FMT: &'static str = "%Y-%m-%d-%H%M%S-%3f";
        let now = chrono::Local::now();
        Cow::Owned(format!("{}.{}", file_args.name_prefix, now.format(FMT)))
    } else {
        Cow::Borrowed(&file_args.name_prefix)
    };

    let filename_suffix: &str = match &file_args.name_suffix {
        Some(v) => v,
        None => "log",
    };

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .filename_prefix(filename_prefix.as_ref())
        .filename_suffix(filename_suffix)
        .rotation(file_args.rolling.clone())
        .max_log_files(30)
        .build(&file_args.directory)
        .with_context(||"build log file appender faile")?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let layer = Layer::new()
        .with_ansi(false)
        .with_timer(timer)
        .with_target(false)
        .with_writer(non_blocking);

    Ok((guard, layer))
}

