use clap::{Args, Parser, Subcommand};
use ssh_forwarder::PortForwarder;

#[derive(Parser, Debug)]
#[command(name = "ssh-forwarder", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Mode,

    #[command(flatten)]
    common: CommonOpts,
}

#[derive(Subcommand, Debug)]
enum Mode {
    Local,
    Remote,
}

#[derive(Args, Debug)]
struct CommonOpts {
    /// --host
    #[arg(long = "ssh-host")]
    host: String,

    #[arg(long = "ssh-port", default_value_t = 22)]
    port: u32,

    /// --user
    #[arg(long = "ssh-user")]
    user: String,

    /// --password
    #[arg(long = "ssh-password")]
    password: String,

    /// --local, -l
    #[arg(long = "local-addr", default_value = "127.0.0.1")]
    local: String,

    #[arg(long = "local-port")]
    lp: String,

    /// --remote, -r
    #[arg(long = "remote-addr")]
    remote: String,

    /// --rp (u32)
    #[arg(long = "remote-port")]
    rp: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let local_addr = format!("{}:{}", cli.common.local, cli.common.lp);
    let ssh_host = format!("{}:{}", cli.common.host, cli.common.port);

    let mut forwarder = PortForwarder::new(
        ssh_host,
        cli.common.user,
        cli.common.password,
        local_addr,
        cli.common.remote,
        cli.common.rp,
    )
    .await?;

    match cli.cmd {
        Mode::Local => forwarder.local_forward().await?,
        Mode::Remote => forwarder.remote_forward().await?,
    }

    Ok(())
}
