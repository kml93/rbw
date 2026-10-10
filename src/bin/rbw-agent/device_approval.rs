use anyhow::Context as _;
use futures_util::{SinkExt as _, StreamExt as _};
use rand::distr::SampleString as _;
use rsa::pkcs8::EncodePublicKey as _;

// matches what the official clients generate
const ACCESS_CODE_LEN: usize = 25;
const RSA_KEY_BITS: usize = 2048;
// both Vaultwarden and Bitwarden drop auth requests after 15 minutes
const AUTH_REQUEST_LIFETIME: std::time::Duration =
    std::time::Duration::from_secs(15 * 60);
// half of the 30 second client timeout of SignalR servers
const KEEPALIVE_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(15);

/// An auth request ("log in with device") waiting for the user to approve
/// it on another device, which then sends the user key encrypted for an
/// ephemeral key pair that never leaves this process.
pub struct PendingApproval {
    auth_request: rbw::api::AuthRequest,
    access_code: String,
    private_key: rsa::RsaPrivateKey,
    fingerprint_phrase: String,
    hub: crate::notifications::WebSocket,
}

impl PendingApproval {
    pub async fn request(email: &str) -> anyhow::Result<Self> {
        let private_key = tokio::task::spawn_blocking(|| {
            rsa::RsaPrivateKey::new(&mut rand_8::rngs::OsRng, RSA_KEY_BITS)
        })
        .await?
        .context("failed to generate auth request key")?;
        let public_key = private_key
            .to_public_key()
            .to_public_key_der()
            .context("failed to encode auth request public key")?;
        let access_code = rand::distr::Alphanumeric
            .sample_string(&mut rand::rng(), ACCESS_CODE_LEN);

        let auth_request = rbw::actions::create_auth_request(
            email,
            public_key.as_bytes(),
            &access_code,
        )
        .await
        .context("failed to create auth request")?;
        // subscribe right away: the server only notifies the approval once
        let hub = connect_hub(&auth_request.id).await?;

        Ok(Self {
            auth_request,
            access_code,
            private_key,
            // approving clients use the email as the server stores it
            fingerprint_phrase: rbw::fingerprint::fingerprint_phrase(
                &email.trim().to_lowercase(),
                public_key.as_bytes(),
            ),
            hub,
        })
    }

    pub fn fingerprint_phrase(&self) -> &str {
        &self.fingerprint_phrase
    }

    /// Waits for the approval and returns the user key. A denied request is
    /// silently deleted by the server, so denial looks like waiting until
    /// the request expires.
    pub async fn approved_user_key(
        mut self,
    ) -> anyhow::Result<rbw::locked::Keys> {
        tokio::time::timeout(
            AUTH_REQUEST_LIFETIME,
            wait_for_approval(&mut self.hub, &self.auth_request.id),
        )
        .await
        .context("auth request expired")??;

        let encrypted_key = rbw::actions::auth_request_key(
            &self.auth_request,
            &self.access_code,
        )
        .await
        .context("failed to fetch approved auth request")?
        .context("auth request was approved without a key")?;
        self.decrypt_user_key(&encrypted_key)
    }

    fn decrypt_user_key(
        &self,
        encrypted_key: &str,
    ) -> anyhow::Result<rbw::locked::Keys> {
        let user_key = rbw::cipherstring::CipherString::new(encrypted_key)?
            .decrypt_locked_rsa(&self.private_key)
            .context("failed to decrypt approved auth request key")?;
        let len = user_key.data().len();
        if len != rbw::locked::Keys::LEN {
            return Err(anyhow::anyhow!(
                "approving device sent a {len} byte key, expected {}",
                rbw::locked::Keys::LEN
            ));
        }
        Ok(rbw::locked::Keys::new(user_key))
    }
}

async fn connect_hub(
    auth_request_id: &str,
) -> anyhow::Result<crate::notifications::WebSocket> {
    let config = rbw::config::Config::load_async().await?;
    let url = format!(
        "{}/anonymous-hub?Token={auth_request_id}",
        config.notifications_websocket_url()
    );
    crate::notifications::connect(&url)
        .await
        .context("failed to connect to the auth request notifications")
}

// Reading the stream answers the server's websocket pings, and the SignalR
// pings keep servers that expect hub level traffic from closing it.
async fn wait_for_approval(
    hub: &mut crate::notifications::WebSocket,
    auth_request_id: &str,
) -> anyhow::Result<()> {
    let mut keepalive = tokio::time::interval(KEEPALIVE_INTERVAL);
    loop {
        tokio::select! {
            message = hub.next() => {
                let message =
                    message.context("auth request notifications closed")??;
                let approved_id =
                    crate::notifications::approved_auth_request_id(message);
                if approved_id.as_deref() == Some(auth_request_id) {
                    return Ok(());
                }
            }
            _ = keepalive.tick() => {
                hub.send(crate::notifications::ping()).await?;
            }
        }
    }
}
