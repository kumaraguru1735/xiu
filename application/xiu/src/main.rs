use {
    //https://rustcc.cn/article?id=6dcbf032-0483-4980-8bfe-c64a7dfb33c7
    anyhow::Result,
    clap::{Arg, Command, value_parser},
    env_logger_extend::logger::{Logger, Rotate},
    std::{env, str::FromStr},
    tokio::signal,
    xiu::{config, service::Service},
};

// #[tokio::main(flavor = "current_thread")]
#[tokio::main]
async fn main() -> Result<()> {
    let log_levels = vec!["trace", "debug", "info", "warn", "error"];

    let mut cmd = Command::new("XIU")
        .bin_name("xiu")
        .version("0.12.7")
        .author("HarlanC <harlanc@foxmail.com>")
        .about("A secure and easy to use live media server, hope you love it!!!")
        .arg(
            Arg::new("config_file_path")
                .long("config")
                .short('c')
                .value_name("path")
                .help("Specify the xiu server configuration file path.")
                .value_parser(value_parser!(String))
        );
    let args: Vec<String> = env::args().collect();
    // if 1 == args.len() {
    //     cmd.print_help()?;
    //     return Ok(());
    // }

    let matches = cmd.clone().get_matches();

    let config = if let Some(path) = matches.get_one::<String>("config_file_path") {
        let config = config::load(path);
        match config {
            Ok(val) => val,
            Err(err) => {
                println!("{path}: {err}");
                return Ok(());
            }
        }
    } else {
        let config = config::load(&"config.toml".to_string());
        match config {
            Ok(val) => val,
            Err(err) => {
                println!("config.toml: {err}");
                println!("Please specify the configuration file path with -c or --config");
                return Ok(());
            }
        }
    };

    /*set log level*/
    let logger = if let Some(log_config_value) = &config.log {
        let (rotate, path) = if let Some(file_info) = &log_config_value.file {
            if file_info.enabled {
                (
                    Some(Rotate::from_str(&file_info.rotate).unwrap()),
                    Some(file_info.path.clone()),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        Logger::new(&log_config_value.level, rotate, path)?
    } else {
        Logger::new(&String::from("info"), None, None)?
    };

    /*run the service*/
    let mut service = Service::new(config);
    service.run().await?;

    // log::info!("log info...");
    // log::warn!("log warn...");
    // log::error!("log err...");
    // log::trace!("log trace...");
    // log::debug!("log debug...");

    signal::ctrl_c().await?;
    logger.stop();
    Ok(())
}
