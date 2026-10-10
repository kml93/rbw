use futures_util::{SinkExt as _, StreamExt as _};

const SIGNALR_HANDSHAKE: &str =
    "{\"protocol\":\"messagepack\",\"version\":1}\x1e";
// length prefixed MessagePack `[6]`
const SIGNALR_PING: &[u8] = &[0x02, 0x91, 0x06];
const RECEIVE_MESSAGE_TARGET: &str = "ReceiveMessage";
// the official server's spelling, which Vaultwarden mirrors
const AUTH_REQUEST_RESPONSE_TARGET: &str = "AuthRequestResponseRecieved";

pub type WebSocket = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

#[derive(Clone, Copy, Debug)]
pub enum Message {
    Sync,
    Logout,
}

pub struct Handler {
    write: Option<
        futures::stream::SplitSink<
            WebSocket,
            tokio_tungstenite::tungstenite::Message,
        >,
    >,
    read_handle: Option<tokio::task::JoinHandle<()>>,
    sending_channels: std::sync::Arc<
        tokio::sync::RwLock<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>,
    >,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            write: None,
            read_handle: None,
            sending_channels: std::sync::Arc::new(tokio::sync::RwLock::new(
                Vec::new(),
            )),
        }
    }

    pub async fn connect(
        &mut self,
        url: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_connected() {
            self.disconnect().await?;
        }

        let (write, read_handle) =
            subscribe_to_notifications(url, self.sending_channels.clone())
                .await?;

        self.write = Some(write);
        self.read_handle = Some(read_handle);
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.write.is_some()
            && self.read_handle.is_some()
            && !self.read_handle.as_ref().unwrap().is_finished()
    }

    pub async fn disconnect(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.sending_channels.write().await.clear();
        if let Some(mut write) = self.write.take() {
            write
                .send(tokio_tungstenite::tungstenite::Message::Close(None))
                .await?;
            write.close().await?;
            self.read_handle.take().unwrap().await?;
        }
        self.write = None;
        self.read_handle = None;
        Ok(())
    }

    pub async fn get_channel(
        &self,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.sending_channels.write().await.push(tx);
        rx
    }
}

async fn subscribe_to_notifications(
    url: String,
    sending_channels: std::sync::Arc<
        tokio::sync::RwLock<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>,
    >,
) -> Result<
    (
        futures_util::stream::SplitSink<
            WebSocket,
            tokio_tungstenite::tungstenite::Message,
        >,
        tokio::task::JoinHandle<()>,
    ),
    Box<dyn std::error::Error>,
> {
    let url = url::Url::parse(url.as_str())?;
    let (write, read) = connect(url.as_str()).await?.split();

    let read_future = async move {
        let sending_channels = &sending_channels;
        read.for_each(|message| async move {
            match message {
                Ok(message) => {
                    if let Some(message) = parse_message(message) {
                        let sending_channels = sending_channels.read().await;
                        let sending_channels = sending_channels.as_slice();
                        for channel in sending_channels {
                            channel.send(message).unwrap();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("websocket error: {e:?}");
                }
            }
        })
        .await;
    };

    Ok((write, tokio::spawn(read_future)))
}

/// Connects to a notifications hub and performs the `SignalR` handshake.
pub async fn connect(
    url: &str,
) -> Result<WebSocket, tokio_tungstenite::tungstenite::Error> {
    let (mut ws_stream, _response) =
        tokio_tungstenite::connect_async(url).await?;
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            SIGNALR_HANDSHAKE.into(),
        ))
        .await?;
    Ok(ws_stream)
}

/// A `SignalR` keep-alive, for hubs that close silent clients.
pub fn ping() -> tokio_tungstenite::tungstenite::Message {
    tokio_tungstenite::tungstenite::Message::Binary(SIGNALR_PING.into())
}

/// Returns the id of the auth request whose approval this message (from
/// the anonymous hub) announces.
pub fn approved_auth_request_id(
    message: tokio_tungstenite::tungstenite::Message,
) -> Option<String> {
    let update = parse_update(message, AUTH_REQUEST_RESPONSE_TARGET)?;
    let payload = map_value(&update, "Payload")?;
    Some(map_value(payload, "Id")?.as_str()?.to_string())
}

fn parse_message(
    message: tokio_tungstenite::tungstenite::Message,
) -> Option<Message> {
    let update = parse_update(message, RECEIVE_MESSAGE_TARGET)?;
    let ty = map_value(&update, "Type")?.as_i64()?;
    match ty {
        11 => Some(Message::Logout),
        _ => Some(Message::Sync),
    }
}

// Returns the update carried by a SignalR MessagePack invocation of
// `target`, i.e. its first argument.
fn parse_update(
    message: tokio_tungstenite::tungstenite::Message,
    target: &str,
) -> Option<rmpv::Value> {
    let tokio_tungstenite::tungstenite::Message::Binary(data) = message
    else {
        return None;
    };

    // the first few bytes with the 0x80 bit set, plus one byte terminating the length contain the length of the message
    let len_buffer_length = data.iter().position(|&x| (x & 0x80) == 0)? + 1;

    let unpacked_messagepack =
        rmpv::decode::read_value(&mut &data[len_buffer_length..]).ok()?;

    let unpacked_message = unpacked_messagepack.as_array()?;
    let message_type = unpacked_message.first()?.as_u64()?;
    // invocation
    if message_type != 1 {
        return None;
    }
    if unpacked_message.get(3)?.as_str()? != target {
        return None;
    }

    unpacked_message.get(4)?.as_array()?.first().cloned()
}

fn map_value<'a>(map: &'a rmpv::Value, key: &str) -> Option<&'a rmpv::Value> {
    map.as_map()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .map(|(_, v)| v)
}
