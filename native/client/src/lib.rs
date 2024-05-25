use anyhow::{Context, Result};
use chacha20poly1305::aead::OsRng;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit};
use ed25519_dalek::SigningKey;
use protocol::bundle::{create_prekey_bundle, sign_bundle};
use protocol::x3dh::{
    x3dh_initiate_recv, x3dh_initiate_send, Message, SignedPreKey, SignedPreKeys,
};
use server::proto::brongnal_client::BrongnalClient;
use server::proto::{
    RegisterPreKeyBundleRequest, RequestPreKeysRequest, RetrieveMessagesRequest,
    SendMessageRequest, X3dhMessage,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Streaming;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

pub trait X3DHClient {
    fn fetch_wipe_one_time_secret_key(
        &mut self,
        one_time_key: &X25519PublicKey,
    ) -> Result<X25519StaticSecret, anyhow::Error>;
    fn get_identity_key(&self) -> Result<SigningKey, anyhow::Error>;
    fn get_pre_key(&mut self) -> Result<X25519StaticSecret, anyhow::Error>;
    fn get_spk(&self) -> Result<SignedPreKey, anyhow::Error>;
    fn add_one_time_keys(&mut self, num_keys: u32) -> SignedPreKeys;
}

struct SessionKeys<T> {
    session_keys: HashMap<T, [u8; 32]>,
}

impl<Identity: Eq + std::hash::Hash> SessionKeys<Identity> {
    fn set_session_key(&mut self, recipient_identity: Identity, secret_key: &[u8; 32]) {
        self.session_keys.insert(recipient_identity, *secret_key);
    }

    fn get_encryption_key(&mut self, recipient_identity: &Identity) -> Result<ChaCha20Poly1305> {
        let key = self
            .session_keys
            .get(recipient_identity)
            .context("Session key not found.")?;
        Ok(ChaCha20Poly1305::new_from_slice(key).unwrap())
    }

    fn destroy_session_key(&mut self, peer: &Identity) {
        self.session_keys.remove(peer);
    }
}

pub struct MemoryClient {
    identity_key: SigningKey,
    pre_key: X25519StaticSecret,
    one_time_pre_keys: HashMap<X25519PublicKey, X25519StaticSecret>,
}

impl Default for MemoryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryClient {
    pub fn new() -> Self {
        Self {
            identity_key: SigningKey::generate(&mut OsRng),
            pre_key: X25519StaticSecret::random_from_rng(OsRng),
            one_time_pre_keys: HashMap::new(),
        }
    }
}

impl X3DHClient for MemoryClient {
    fn fetch_wipe_one_time_secret_key(
        &mut self,
        one_time_key: &X25519PublicKey,
    ) -> Result<X25519StaticSecret> {
        self.one_time_pre_keys
            .remove(one_time_key)
            .context("Client failed to find pre key.")
    }

    fn get_identity_key(&self) -> Result<SigningKey> {
        Ok(self.identity_key.clone())
    }

    fn get_pre_key(&mut self) -> Result<X25519StaticSecret> {
        Ok(self.pre_key.clone())
    }

    fn get_spk(&self) -> Result<SignedPreKey> {
        Ok(SignedPreKey {
            pre_key: X25519PublicKey::from(&self.pre_key),
            signature: sign_bundle(
                &self.identity_key,
                &[(self.pre_key.clone(), X25519PublicKey::from(&self.pre_key))],
            ),
        })
    }

    fn add_one_time_keys(&mut self, num_keys: u32) -> SignedPreKeys {
        let otks = create_prekey_bundle(&self.identity_key, num_keys);
        let pre_keys = otks.bundle.iter().map(|(_, _pub)| *_pub).collect();
        for otk in otks.bundle {
            self.one_time_pre_keys.insert(otk.1, otk.0);
        }
        SignedPreKeys {
            pre_keys,
            signature: otks.signature,
        }
    }
}

pub async fn listen(
    mut stub: BrongnalClient<Channel>,
    x3dh_client: Arc<Mutex<MemoryClient>>,
    name: String,
    tx: Sender<Vec<u8>>,
) -> Result<()> {
    let stream = stub
        .retrieve_messages(RetrieveMessagesRequest {
            identity: Some(name),
        })
        .await?
        .into_inner();
    get_messages(stream, x3dh_client, tx).await?;
    println!("Streaming messages from server.");
    Ok(())
}

pub async fn register(
    stub: &mut BrongnalClient<Channel>,
    x3dh_client: Arc<Mutex<MemoryClient>>,
    name: String,
) -> Result<()> {
    println!("Registering {name}!");
    let request = {
        let mut x3dh_client = x3dh_client.lock().await;
        let ik = x3dh_client
            .get_identity_key()?
            .verifying_key()
            .as_bytes()
            .to_vec();
        let spk = x3dh_client.get_spk()?;
        let otk_bundle = x3dh_client.add_one_time_keys(100);
        tonic::Request::new(RegisterPreKeyBundleRequest {
            ik: Some(ik),
            identity: Some(name.clone()),
            spk: Some(spk.into()),
            otk_bundle: Some(otk_bundle.into()),
        })
    };
    stub.register_pre_key_bundle(request).await?;
    println!("Registered: {}!", name);
    Ok(())
}

pub async fn message(
    stub: &mut BrongnalClient<Channel>,
    x3dh_client: Arc<Mutex<MemoryClient>>,
    name: &str,
    message: &str,
) -> Result<()> {
    let message = message.as_bytes();
    println!("Messaging {name}.");
    let request = tonic::Request::new(RequestPreKeysRequest {
        identity: Some(name.to_owned()),
    });
    let response = stub.request_pre_keys(request).await?;
    let (_sk, message) = x3dh_initiate_send(
        response.into_inner().try_into()?,
        &x3dh_client.lock().await.get_identity_key()?,
        message,
    )?;
    let request = tonic::Request::new(SendMessageRequest {
        recipient_identity: Some(name.to_owned()),
        message: Some(message.into()),
    });
    stub.send_message(request).await?;
    println!("Message Sent!");
    Ok(())
}

// TODO Replace with stream of decrypted messages.
pub async fn get_messages(
    mut stream: Streaming<X3dhMessage>,
    x3dh_client: Arc<Mutex<MemoryClient>>,
    tx: Sender<Vec<u8>>,
) -> Result<()> {
    while let Some(message) = stream.message().await? {
        let Message {
            sender_identity_key,
            ephemeral_key,
            otk,
            ciphertext,
        } = message.try_into()?;
        let mut x3dh_client = x3dh_client.lock().await;
        let otk = if let Some(otk) = otk {
            Some(x3dh_client.fetch_wipe_one_time_secret_key(&otk)?)
        } else {
            None
        };
        let (_sk, message) = x3dh_initiate_recv(
            &x3dh_client.get_identity_key()?,
            &x3dh_client.get_pre_key()?,
            &sender_identity_key,
            ephemeral_key,
            otk,
            &ciphertext,
        )?;
        tx.send(message).await?;
    }
    Ok(())
}
