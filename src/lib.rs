use anyhow::anyhow;
use russh::client::Handle;
use russh::client::{self, AuthResult, Config, Handler};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncWriteExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};

struct Client {
    local: String,
}

impl Client {
    fn new(local: String) -> Self {
        Client { local }
    }
}

impl Handler for Client {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async move { Ok(true) }
    }

    fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        _session: &mut client::Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let target = self.local.clone();
        async move {
            tokio::spawn(async move {
                let mut ssh = channel.into_stream();
                match TcpStream::connect(&target).await {
                    Ok(mut tcp) => {
                        let _ = copy_bidirectional(&mut ssh, &mut tcp).await;
                        let _ = tcp.shutdown().await;
                    }
                    Err(e) => {
                        eprintln!("local connect failed: {e}");
                    }
                }
            });

            Ok(())
        }
    }
}

pub struct PortForwarder {
    remote_ip: &'static str,
    remote_port: u32,
    handle: Handle<Client>,
    local_addr: &'static str,
}

impl PortForwarder {
    pub async fn new(
        ssh_host: &'static str,
        ssh_user: &'static str,
        ssh_password: &'static str,
        local_addr: &'static str,
        remote_ip: &'static str,
        remote_port: u32,
    ) -> anyhow::Result<Self> {
        let mut config = Config::default();
        config.keepalive_interval = Some(Duration::from_secs(30));
        config.keepalive_max = 4;

        let client = Client::new(local_addr.to_string());

        let mut handle = client::connect(Arc::new(config), ssh_host, client).await?;

        match handle.authenticate_password(ssh_user, ssh_password).await? {
            AuthResult::Success => {}
            other => return Err(anyhow!("Auth failed: {:?}", other)),
        };

        Ok(PortForwarder {
            remote_ip,
            remote_port,
            handle,
            local_addr,
        })
    }

    pub async fn local_forward(&mut self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.local_addr).await?;
        loop {
            let (mut inbound, peer) = match listener.accept().await {
                Ok(x) => x,
                Err(e) => {
                    eprint!("accept error: {e}");
                    continue;
                }
            };
            let orig_port = peer.port() as u32;
            let orig_addr = peer.ip().to_string();
            let chan = self
                .handle
                .channel_open_direct_tcpip(self.remote_ip, self.remote_port, orig_addr, orig_port)
                .await?;
            let mut ch_stream = chan.into_stream();

            if let Err(e) = copy_bidirectional(&mut inbound, &mut ch_stream).await {
                eprint!("pipe error: {e}");
            };
        }
    }

    pub async fn remote_forward(&mut self) -> anyhow::Result<()> {
        let port = self
            .handle
            .tcpip_forward(self.remote_ip, self.remote_port)
            .await?;
        dbg!(port);

        tokio::signal::ctrl_c().await?;
        self.handle
            .cancel_tcpip_forward(self.remote_ip, self.remote_port)
            .await?;
        self.handle
            .disconnect(russh::Disconnect::ByApplication, "bye", "")
            .await?;

        Ok(())
    }
}