/// 获取运行时环境变量及输入参数
///
/// - `-c | --config`: 配置文件路径
pub fn arguments() -> clap::ArgMatches {
    clap::Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("配置文件路径")
                .required(false),
        )
        .get_matches()
}
