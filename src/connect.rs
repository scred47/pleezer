use crate::{config::Config, token};

pub struct Connect {
    provider: Box<dyn token::UserTokenProvider>,
}

impl Connect {
    pub async fn new<P>(
        config: &Config,
        mut provider: P,
        secure: bool,
    ) -> Result<Self, token::UserTokenError>
    where
        P: token::UserTokenProvider + 'static,
    {
        let scheme = if secure { "wss" } else { "ws" };
        let token = provider.user_token().await?;

        let version = config.app_version.replace('.', "");
        for chr in version.chars() {
            if !chr.is_digit(10) {
                std::process::exit(1); // TODO
            }
        }

        //        let version =

        let url = format!("{scheme}://live.deezer.com/ws/{token}?version={version}");

        // TODO : refresh token (maybe on drop?)

        tokio_tungstenite::connect_async(url)
            .await
            .expect("Failed to connect"); // FIXME <-- turn into err and print something useful
       // while true {}

        Ok(Self {
            provider: Box::new(provider),
        })
    }
}

impl Drop for Connect {
    fn drop(&mut self) {
        trace!("dropping connect client");
        self.provider.expire_token();
    }
}
