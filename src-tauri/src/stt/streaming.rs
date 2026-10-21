//! Shared engine for cloud providers that stream audio over a WebSocket and stream transcript
//! JSON back (Deepgram, AssemblyAI). Only the URL, auth header, close message and the
//! response parser differ per provider.

use super::{SpeechToText, TranscriptEvent};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// Turns one text message from the provider into transcript events.
pub trait StreamParser: Send + 'static {
    /// `Err` means the provider reported a failure for this session.
    fn parse(&mut self, message: &str) -> Result<Vec<TranscriptEvent>, String>;
}

pub struct StreamConfig {
    pub provider: &'static str,
    pub url: String,
    pub authorization: String,
    /// Text message asking the provider to flush and end the session.
    pub close_message: &'static str,
}

const CHUNK_SAMPLES: usize = 1_600; // 100 ms at 16 kHz
const FINISH_TIMEOUT: Duration = Duration::from_secs(8);

enum Outgoing {
    Audio(Vec<u8>),
    Finish,
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

pub struct StreamingEngine {
    provider: &'static str,
    audio_tx: Sender<Outgoing>,
    events_rx: Receiver<Result<Vec<TranscriptEvent>, String>>,
    pending: Vec<u8>,
    io: Option<JoinHandle<()>>,
}

impl StreamingEngine {
    /// Connects synchronously so a bad key or network problem fails `start_recording` with a
    /// clear message instead of surfacing mid-meeting.
    pub fn connect(config: StreamConfig, parser: impl StreamParser) -> Result<Self, String> {
        let provider = config.provider;
        let mut request = config
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| format!("{provider}: bad URL: {e}"))?;
        request.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&config.authorization)
                .map_err(|_| format!("{provider}: API key contains invalid characters"))?,
        );
        let (mut socket, _) =
            tungstenite::connect(request).map_err(|e| describe_connect_error(provider, e))?;
        set_read_timeout(&mut socket, Duration::from_millis(20))?;

        let (audio_tx, audio_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let io = std::thread::spawn(move || {
            run_io(
                socket,
                audio_rx,
                events_tx,
                parser,
                config.close_message,
                provider,
            )
        });
        Ok(Self {
            provider,
            audio_tx,
            events_rx,
            pending: Vec::with_capacity(CHUNK_SAMPLES * 2),
            io: Some(io),
        })
    }

    fn drain(&mut self) -> Result<Vec<TranscriptEvent>, String> {
        let mut out = Vec::new();
        loop {
            match self.events_rx.try_recv() {
                Ok(batch) => out.extend(batch?),
                Err(TryRecvError::Empty) => return Ok(out),
                Err(TryRecvError::Disconnected) => {
                    return if out.is_empty() {
                        Err(format!("{} connection closed", self.provider))
                    } else {
                        Ok(out)
                    };
                }
            }
        }
    }
}

impl SpeechToText for StreamingEngine {
    fn push(&mut self, samples: &[f32]) -> Result<Vec<TranscriptEvent>, String> {
        self.pending.extend(pcm16(samples));
        while self.pending.len() >= CHUNK_SAMPLES * 2 {
            let chunk: Vec<u8> = self.pending.drain(..CHUNK_SAMPLES * 2).collect();
            if self.audio_tx.send(Outgoing::Audio(chunk)).is_err() {
                break;
            }
        }
        self.drain()
    }

    fn finish(&mut self) -> Result<Vec<TranscriptEvent>, String> {
        if !self.pending.is_empty() {
            let _ = self
                .audio_tx
                .send(Outgoing::Audio(std::mem::take(&mut self.pending)));
        }
        let _ = self.audio_tx.send(Outgoing::Finish);
        let deadline = Instant::now() + FINISH_TIMEOUT;
        let mut out = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events_rx.recv_timeout(left) {
                Ok(batch) => out.extend(batch?),
                Err(RecvTimeoutError::Disconnected) | Err(RecvTimeoutError::Timeout) => break,
            }
        }
        if let Some(io) = self.io.take() {
            let _ = io.join();
        }
        Ok(out)
    }
}

/// f32 [-1, 1] to 16-bit little-endian PCM, the format both providers accept.
pub fn pcm16(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes())
        .collect()
}

fn set_read_timeout(socket: &mut Socket, timeout: Duration) -> Result<(), String> {
    let tcp = match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_mut(),
        _ => return Ok(()),
    };
    tcp.set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())
}

fn describe_connect_error(provider: &str, e: tungstenite::Error) -> String {
    match e {
        tungstenite::Error::Http(response) => match response.status().as_u16() {
            401 | 403 => format!("{provider} rejected the API key ({})", response.status()),
            code => format!("{provider} refused the connection (HTTP {code})"),
        },
        other => format!("could not reach {provider}: {other}"),
    }
}

/// Owns the socket: forwards audio, polls for transcript messages, and after `Finish` keeps
/// reading until the provider closes or the finish timeout passes.
fn run_io(
    mut socket: Socket,
    audio_rx: Receiver<Outgoing>,
    events_tx: Sender<Result<Vec<TranscriptEvent>, String>>,
    mut parser: impl StreamParser,
    close_message: &'static str,
    provider: &'static str,
) {
    let mut finish_by: Option<Instant> = None;
    loop {
        if finish_by.is_none() {
            loop {
                match audio_rx.try_recv() {
                    Ok(Outgoing::Audio(bytes)) => {
                        if let Err(e) = socket.send(Message::Binary(bytes.into())) {
                            let _ = events_tx.send(Err(format!("{provider}: {e}")));
                            return;
                        }
                    }
                    Ok(Outgoing::Finish) | Err(TryRecvError::Disconnected) => {
                        let _ = socket.send(Message::Text(close_message.into()));
                        finish_by = Some(Instant::now() + FINISH_TIMEOUT);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }
        if finish_by.is_some_and(|t| Instant::now() > t) {
            let _ = socket.close(None);
            return;
        }
        match socket.read() {
            Ok(Message::Text(text)) => match parser.parse(text.as_str()) {
                Ok(events) if events.is_empty() => {}
                Ok(events) => {
                    let _ = events_tx.send(Ok(events));
                }
                Err(e) => {
                    let _ = events_tx.send(Err(format!("{provider}: {e}")));
                    return;
                }
            },
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => return,
            Err(e) => {
                if finish_by.is_none() {
                    let _ = events_tx.send(Err(format!("{provider}: {e}")));
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Echoes the byte count of each audio frame back as a partial, and on the close message
    /// sends a final with the total and closes.
    struct Counting;

    impl StreamParser for Counting {
        fn parse(&mut self, message: &str) -> Result<Vec<TranscriptEvent>, String> {
            let v: serde_json::Value = serde_json::from_str(message).map_err(|e| e.to_string())?;
            if let Some(err) = v["error"].as_str() {
                return Err(err.to_string());
            }
            Ok(vec![TranscriptEvent {
                segment_id: 0,
                is_final: v["final"].as_bool().unwrap_or(false),
                text: v["bytes"].to_string(),
                start_ms: 0,
                end_ms: 0,
            }])
        }
    }

    // tungstenite's accept_hdr callback signature fixes the (large) error type.
    #[allow(clippy::result_large_err)]
    fn fake_provider(error_after: Option<usize>) -> (String, JoinHandle<(usize, Option<String>)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}/listen", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut auth = None;
            let mut ws = tungstenite::accept_hdr(
                stream,
                |req: &tungstenite::handshake::server::Request, resp| {
                    auth = req
                        .headers()
                        .get("Authorization")
                        .map(|v| v.to_str().unwrap().to_string());
                    Ok(resp)
                },
            )
            .unwrap();
            let mut total = 0;
            let mut frames = 0;
            loop {
                match ws.read() {
                    Ok(Message::Binary(b)) => {
                        total += b.len();
                        frames += 1;
                        if Some(frames) == error_after {
                            ws.send(Message::Text(r#"{"error":"quota exceeded"}"#.into()))
                                .unwrap();
                            continue;
                        }
                        ws.send(Message::Text(format!(r#"{{"bytes":{total}}}"#).into()))
                            .unwrap();
                    }
                    Ok(Message::Text(t)) if t.as_str() == r#"{"type":"Done"}"# => {
                        ws.send(Message::Text(
                            format!(r#"{{"bytes":{total},"final":true}}"#).into(),
                        ))
                        .unwrap();
                        let _ = ws.close(None);
                        let _ = ws.flush();
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            (total, auth)
        });
        (url, server)
    }

    fn config(url: String) -> StreamConfig {
        StreamConfig {
            provider: "Fake",
            url,
            authorization: "Token test-key".into(),
            close_message: r#"{"type":"Done"}"#,
        }
    }

    #[test]
    fn streams_pcm_in_100ms_chunks_and_collects_the_final_on_finish() {
        let (url, server) = fake_provider(None);
        let mut engine = StreamingEngine::connect(config(url), Counting).unwrap();
        let mut events = Vec::new();
        for _ in 0..10 {
            events.extend(engine.push(&vec![0.1; 800]).unwrap());
            std::thread::sleep(Duration::from_millis(5));
        }
        events.extend(engine.finish().unwrap());
        let (total, auth) = server.join().unwrap();

        assert_eq!(total, 8_000 * 2, "every sample arrives as 16-bit PCM");
        assert_eq!(auth.as_deref(), Some("Token test-key"));
        let last = events.last().expect("final event");
        assert!(last.is_final);
        assert_eq!(last.text, "16000");
    }

    #[test]
    fn provider_errors_surface_from_push() {
        let (url, server) = fake_provider(Some(1));
        let mut engine = StreamingEngine::connect(config(url), Counting).unwrap();
        let mut result = Ok(vec![]);
        for _ in 0..50 {
            result = engine.push(&vec![0.0; CHUNK_SAMPLES]);
            if result.is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(result.unwrap_err(), "Fake: quota exceeded");
        drop(engine);
        drop(server);
    }

    #[test]
    fn connection_refused_is_a_clear_error() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = StreamingEngine::connect(config(format!("ws://127.0.0.1:{port}/x")), Counting)
            .err()
            .unwrap();
        assert!(err.starts_with("could not reach Fake"), "{err}");
    }

    #[test]
    fn pcm16_scales_and_clamps() {
        assert_eq!(
            pcm16(&[0.0, 1.0, -1.0, 2.0]),
            [0, 0, 0xff, 0x7f, 0x01, 0x80, 0xff, 0x7f]
        );
    }
}
