pub mod payload;
pub mod types;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message<T = serde_json::Value> {
	#[serde(rename = "d")]
	pub data: T,
	#[serde(rename = "op")]
	pub opcode: types::Opcode,
	#[serde(rename = "t")]
	#[serde(default)]
	pub timestamp: i64,
	#[serde(rename = "s")]
	#[serde(default)]
	pub sequence: u64,
}

pub trait MessagePayload {
	fn opcode(&self) -> types::Opcode;
}

impl<T: MessagePayload> MessagePayload for &T {
	fn opcode(&self) -> types::Opcode {
		(*self).opcode()
	}
}

impl<T: MessagePayload> Message<T> {
	pub fn new(data: T, seq: u64) -> Self {
		Self {
			opcode: data.opcode(),
			data,
			timestamp: chrono::Utc::now().timestamp_millis(),
			sequence: seq,
		}
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum MessageData {
	Hello(payload::Hello),
	Ack(payload::Ack),
	Heartbeat(payload::Heartbeat),
	Reconnect(payload::Reconnect),
	Resume(payload::Resume),
	Subscribe(payload::Subscribe),
	Unsubscribe(payload::Unsubscribe),
	Dispatch(Box<payload::Dispatch>),
	Signal(Box<payload::Signal>),
	Error(payload::Error),
	EndOfStream(payload::EndOfStream),
	Bridge(payload::Bridge),
}

impl MessagePayload for MessageData {
	fn opcode(&self) -> types::Opcode {
		match self {
			Self::Hello(payload) => payload.opcode(),
			Self::Ack(payload) => payload.opcode(),
			Self::Heartbeat(payload) => payload.opcode(),
			Self::Reconnect(payload) => payload.opcode(),
			Self::Resume(payload) => payload.opcode(),
			Self::Subscribe(payload) => payload.opcode(),
			Self::Unsubscribe(payload) => payload.opcode(),
			Self::Dispatch(payload) => payload.opcode(),
			Self::Signal(payload) => payload.opcode(),
			Self::Error(payload) => payload.opcode(),
			Self::EndOfStream(payload) => payload.opcode(),
			Self::Bridge(payload) => payload.opcode(),
		}
	}
}

/// Shorthand macro for implementing `From` for `MessageData`.
macro_rules! impl_from {
    ($($ty:ident),*) => {
        $(
            impl From<payload::$ty> for MessageData {
                fn from(data: payload::$ty) -> Self {
                    Self::$ty(data.into())
                }
            }
        )*
    };
}

impl_from!(
	Hello,
	Ack,
	Heartbeat,
	Reconnect,
	Resume,
	Subscribe,
	Unsubscribe,
	Dispatch,
	Signal,
	Error,
	EndOfStream,
	Bridge
);
