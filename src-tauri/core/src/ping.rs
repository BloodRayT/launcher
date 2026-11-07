//! Опрос Minecraft-сервера по протоколу Server List Ping: сколько игроков онлайн сейчас.
use serde_json::Value;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// «Любая версия протокола»: серверы отвечают на статус-запрос независимо от неё.
const ANY_PROTOCOL: i32 = -1;
const STATUS_STATE: i32 = 1;
const MAX_STATUS_BYTES: usize = 32 * 1024;
/// Общий срок на весь ответ: сервер, присылающий байт раз в секунду, не задержит обновление онлайна.
const STATUS_DEADLINE: Duration = Duration::from_secs(5);

/// Чтение, которое прекращается, когда истёк общий срок, а не только время одного чтения.
struct Deadline<'a, R> {
    inner: &'a mut R,
    until: Instant,
}

impl<R: Read> Read for Deadline<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if Instant::now() >= self.until {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "сервер отвечает слишком долго",
            ));
        }
        self.inner.read(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCount {
    pub online: u32,
    pub max: u32,
}

pub fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut rest = value as u32;
    loop {
        let byte = (rest & 0x7F) as u8;
        rest >>= 7;
        if rest == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub fn read_varint(input: &mut impl Read) -> std::io::Result<i32> {
    let mut value: u32 = 0;
    for shift in (0..35).step_by(7) {
        let mut byte = [0u8; 1];
        input.read_exact(&mut byte)?;
        value |= u32::from(byte[0] & 0x7F) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "слишком длинное число в ответе сервера",
    ))
}

fn framed(body: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(body.len() + 5);
    write_varint(&mut packet, body.len() as i32);
    packet.extend_from_slice(body);
    packet
}

/// Рукопожатие с переходом в режим статуса и сам статус-запрос одним буфером.
pub fn status_request(host: &str, port: u16) -> Vec<u8> {
    let mut handshake = vec![0x00];
    write_varint(&mut handshake, ANY_PROTOCOL);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, STATUS_STATE);
    let mut request = framed(&handshake);
    request.extend(framed(&[0x00]));
    request
}

/// Число игроков из JSON статуса: `{"players":{"online":3,"max":20}}`.
pub fn players_from_status(json: &str) -> Option<PlayerCount> {
    let status: Value = serde_json::from_str(json).ok()?;
    let players = &status["players"];
    Some(PlayerCount {
        online: u32::try_from(players["online"].as_u64()?).ok()?,
        max: u32::try_from(players["max"].as_u64()?).ok()?,
    })
}

/// Отправляет статус-запрос в открытое соединение и читает ответ.
pub fn query_players(
    stream: &mut (impl Read + Write),
    host: &str,
    port: u16,
) -> Option<PlayerCount> {
    stream.write_all(&status_request(host, port)).ok()?;
    query_reply(stream, Instant::now() + STATUS_DEADLINE)
}

fn query_reply(stream: &mut impl Read, until: Instant) -> Option<PlayerCount> {
    let mut reader = Deadline {
        inner: stream,
        until,
    };
    let _length = read_varint(&mut reader).ok()?;
    if read_varint(&mut reader).ok()? != 0x00 {
        return None;
    }
    let size = usize::try_from(read_varint(&mut reader).ok()?).ok()?;
    if size > MAX_STATUS_BYTES {
        return None;
    }
    let mut json = vec![0u8; size];
    reader.read_exact(&mut json).ok()?;
    players_from_status(&String::from_utf8_lossy(&json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn varints_round_trip_including_negative_numbers() {
        for value in [0, 1, 127, 128, 25565, 2_097_151, -1] {
            let mut bytes = Vec::new();
            write_varint(&mut bytes, value);
            assert_eq!(read_varint(&mut Cursor::new(bytes)).unwrap(), value);
        }
        let mut bytes = Vec::new();
        write_varint(&mut bytes, -1);
        assert_eq!(bytes, [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn status_request_contains_host_port_and_status_state() {
        let request = status_request("play.fenixcraft.ru", 25565);
        let mut cursor = Cursor::new(&request);
        let length = read_varint(&mut cursor).unwrap() as usize;
        let handshake = &request[cursor.position() as usize..][..length];

        assert_eq!(handshake[0], 0x00);
        assert!(handshake
            .windows(18)
            .any(|part| part == b"play.fenixcraft.ru"));
        assert_eq!(&handshake[length - 3..length - 1], &25565u16.to_be_bytes());
        assert_eq!(handshake[length - 1], 1);
        assert_eq!(&request[request.len() - 2..], &[0x01, 0x00]);
    }

    #[test]
    fn player_count_is_read_from_the_status_json() {
        let json = r#"{"version":{"name":"26.2"},"players":{"online":7,"max":20},"description":"FenixCraft"}"#;
        assert_eq!(
            players_from_status(json),
            Some(PlayerCount { online: 7, max: 20 })
        );
        assert_eq!(players_from_status(r#"{"description":"no players"}"#), None);
        assert_eq!(players_from_status("not json"), None);
    }

    struct FakeServer {
        reply: Cursor<Vec<u8>>,
        sent: Vec<u8>,
    }

    impl Read for FakeServer {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.reply.read(buf)
        }
    }

    impl Write for FakeServer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.sent.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn reply_past_the_deadline_is_abandoned() {
        let json = br#"{"players":{"online":1,"max":2}}"#;
        let mut body = vec![0x00];
        write_varint(&mut body, json.len() as i32);
        body.extend_from_slice(json);
        let mut reply = Cursor::new(framed(&body));

        assert_eq!(query_reply(&mut reply, Instant::now()), None);
    }

    #[test]
    fn players_are_read_from_a_full_server_reply() {
        let json = br#"{"players":{"online":12,"max":100}}"#;
        let mut body = vec![0x00];
        write_varint(&mut body, json.len() as i32);
        body.extend_from_slice(json);
        let mut server = FakeServer {
            reply: Cursor::new(framed(&body)),
            sent: Vec::new(),
        };

        assert_eq!(
            query_players(&mut server, "localhost", 25565),
            Some(PlayerCount {
                online: 12,
                max: 100
            })
        );
        assert_eq!(server.sent, status_request("localhost", 25565));
    }
}
