use crate::config::{ProjectDirsExt, LAUNCHER_DIRECTORY};
use crate::utils::security_utils::mask_sensitive_data;
use log::{LevelFilter, Record};
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::encode::writer::simple::SimpleWriter;
use log4rs::encode::{Encode, Write};
use std::sync::OnceLock;
use tokio::fs;

const LOG_DIR_NAME: &str = "logs";
const LOG_FILE_NAME: &str = "launcher.log";
const LOG_PATTERN: &str = "{d(%Y-%m-%d %H:%M:%S%.3f)} | {({l}):5.5} | {m}{n}";
const CONSOLE_LOG_PATTERN: &str = "{d(%H:%M:%S)} | {h({l}):5.5} | {m}{n}";
const LOG_FILE_SIZE_LIMIT_MB: u64 = 10;
const LOG_FILE_BACKUP_COUNT: u32 = 5;

pub const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Debug;

static LOG_HANDLE: OnceLock<log4rs::Handle> = OnceLock::new();
static LOG_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

#[derive(Debug)]
struct RedactingEncoder {
    inner: PatternEncoder,
}

impl RedactingEncoder {
    fn new(pattern: &str) -> Self {
        Self {
            inner: PatternEncoder::new(pattern),
        }
    }
}

impl Encode for RedactingEncoder {
    fn encode(&self, w: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        let mut rendered = Vec::new();
        self.inner
            .encode(&mut SimpleWriter(&mut rendered), record)?;
        let line = String::from_utf8_lossy(&rendered);
        w.write_all(mask_sensitive_data(&line).as_bytes())?;
        Ok(())
    }
}

pub fn env_log_level() -> Option<LevelFilter> {
    std::env::var("VXL_LOG_LEVEL")
        .ok()
        .and_then(|value| value.trim().parse::<LevelFilter>().ok())
}

pub fn current_log_level() -> LevelFilter {
    log::max_level()
}

fn build_config(log_dir: &std::path::Path, level: LevelFilter) -> anyhow::Result<Config> {
    let log_file_path = log_dir.join(LOG_FILE_NAME);

    let size_trigger = SizeTrigger::new(LOG_FILE_SIZE_LIMIT_MB * 1024 * 1024);
    let roller_pattern = log_dir.join(format!("{}.{{}}", LOG_FILE_NAME));
    let roller = FixedWindowRoller::builder()
        .base(1)
        .build(roller_pattern.to_str().unwrap(), LOG_FILE_BACKUP_COUNT)?;
    let compound_policy = CompoundPolicy::new(Box::new(size_trigger), Box::new(roller));

    let file_appender = RollingFileAppender::builder()
        .encoder(Box::new(RedactingEncoder::new(LOG_PATTERN)))
        .build(log_file_path, Box::new(compound_policy))?;

    let console_appender = ConsoleAppender::builder()
        .encoder(Box::new(RedactingEncoder::new(CONSOLE_LOG_PATTERN)))
        .target(Target::Stdout)
        .build();

    Ok(Config::builder()
        .appender(Appender::builder().build("file", Box::new(file_appender)))
        .appender(Appender::builder().build("stdout", Box::new(console_appender)))
        .logger(Logger::builder().build("sqlx::query", LevelFilter::Warn))
        .logger(Logger::builder().build("hyper", LevelFilter::Info))
        .logger(Logger::builder().build("hyper_util", LevelFilter::Info))
        .logger(Logger::builder().build("reqwest", LevelFilter::Info))
        .build(
            Root::builder()
                .appender("file")
                .appender("stdout")
                .build(level),
        )?)
}

pub async fn setup_logging() -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = LAUNCHER_DIRECTORY.root_dir().join(LOG_DIR_NAME);

    if !log_dir.exists() {
        fs::create_dir_all(&log_dir).await?;
    }

    let from_env = env_log_level();
    let root_level = from_env.unwrap_or(DEFAULT_LOG_LEVEL);
    let config = build_config(&log_dir, root_level)?;
    let handle = log4rs::init_config(config)?;
    let _ = LOG_HANDLE.set(handle);
    let _ = LOG_DIR.set(log_dir.clone());

    log::info!(
        "Logging initialized (level: {}{}). Log directory: {}",
        root_level,
        if from_env.is_some() {
            ", from VXL_LOG_LEVEL"
        } else {
            ""
        },
        log_dir.display()
    );

    Ok(())
}

pub fn set_log_level(level: LevelFilter) {
    let was = current_log_level();
    let Some(handle) = LOG_HANDLE.get() else {
        log::error!("Failed to change log level: logging not initialized");
        return;
    };
    let Some(log_dir) = LOG_DIR.get() else {
        return;
    };
    match build_config(log_dir, level) {
        Ok(config) => {
            handle.set_config(config);
            if was != level {
                log::info!("Log level changed: {} -> {}", was, level);
            }
        }
        Err(e) => log::error!("Failed to rebuild logging config for level {}: {}", level, e),
    }
}
