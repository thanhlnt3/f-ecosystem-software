//! Tra bản ghi DNS TXT để xác minh tên miền của tổ chức (spec developer-portal, "Đăng ký nhà
//! phát triển"). Hỏi lần lượt các máy chủ DNS trong cấu hình (`FECO_DNS_SERVERS`) qua UDP, chuyển
//! sang TCP khi câu trả lời bị cắt.
//!
//! Chỉ dùng cho việc xác minh tên miền: mỗi truy vấn có mã ngẫu nhiên, cổng nguồn do hệ điều
//! hành chọn, và câu trả lời phải lặp lại đúng câu hỏi. Không kiểm DNSSEC (xem QUESTIONS.md).

use std::{net::SocketAddr, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

const TIMEOUT: Duration = Duration::from_secs(3);
const TYPE_TXT: u16 = 16;

#[derive(Debug, thiserror::Error)]
pub enum DnsError {
    #[error("tên miền không hợp lệ")]
    InvalidName,
    #[error("không máy chủ DNS nào trả lời: {0}")]
    Unreachable(String),
}

/// Tên miền hợp lệ để xác minh: các nhãn chữ thường, số, `-`; có ít nhất một dấu chấm.
pub fn valid_domain(domain: &str) -> bool {
    domain.len() <= 253
        && domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

fn query(id: u16, domain: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(32 + domain.len());
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00]); // RD
    q.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]); // 1 câu hỏi
    for label in domain.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&TYPE_TXT.to_be_bytes());
    q.extend_from_slice(&[0, 1]); // IN
    q
}

/// Bỏ qua một tên (nhãn hoặc con trỏ nén); trả vị trí sau tên.
fn skip_name(msg: &[u8], mut at: usize) -> Option<usize> {
    loop {
        let len = *msg.get(at)? as usize;
        if len & 0xc0 == 0xc0 {
            return Some(at + 2);
        }
        if len == 0 {
            return Some(at + 1);
        }
        at += 1 + len;
    }
}

enum Parsed {
    Records(Vec<String>),
    Truncated,
}

/// Đọc câu trả lời; `None` nếu không khớp truy vấn hoặc sai định dạng.
fn parse(msg: &[u8], sent: &[u8]) -> Option<Parsed> {
    if msg.get(..2)? != &sent[..2] || msg.get(2)? & 0x80 == 0 {
        return None;
    }
    if msg[2] & 0x02 != 0 {
        return Some(Parsed::Truncated);
    }
    let questions = u16::from_be_bytes([msg[4], msg[5]]);
    let answers = u16::from_be_bytes([msg[6], msg[7]]);
    // Câu hỏi phải được lặp lại nguyên văn.
    if questions != 1 || msg.get(12..sent.len())? != &sent[12..] {
        return None;
    }
    let mut at = sent.len();
    let mut out = Vec::new();
    for _ in 0..answers {
        at = skip_name(msg, at)?;
        let kind = u16::from_be_bytes([*msg.get(at)?, *msg.get(at + 1)?]);
        let length = u16::from_be_bytes([*msg.get(at + 8)?, *msg.get(at + 9)?]) as usize;
        let data = msg.get(at + 10..at + 10 + length)?;
        at += 10 + length;
        if kind == TYPE_TXT {
            // Một bản ghi TXT gồm nhiều chuỗi có tiền tố độ dài, ghép lại.
            let mut text = Vec::new();
            let mut i = 0;
            while i < data.len() {
                let n = data[i] as usize;
                text.extend_from_slice(data.get(i + 1..i + 1 + n)?);
                i += 1 + n;
            }
            out.push(String::from_utf8_lossy(&text).into_owned());
        }
    }
    Some(Parsed::Records(out))
}

async fn ask(server: SocketAddr, sent: &[u8]) -> Result<Vec<String>, String> {
    let bind = if server.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await.map_err(|e| e.to_string())?;
    socket.connect(server).await.map_err(|e| e.to_string())?;
    socket.send(sent).await.map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(TIMEOUT, socket.recv(&mut buf))
        .await
        .map_err(|_| "hết thời gian chờ".to_owned())?
        .map_err(|e| e.to_string())?;
    match parse(&buf[..n], sent).ok_or("câu trả lời không hợp lệ")? {
        Parsed::Records(r) => Ok(r),
        Parsed::Truncated => {
            let tcp = async {
                let mut stream = TcpStream::connect(server).await?;
                stream.write_all(&(sent.len() as u16).to_be_bytes()).await?;
                stream.write_all(sent).await?;
                let mut len = [0u8; 2];
                stream.read_exact(&mut len).await?;
                let mut msg = vec![0u8; u16::from_be_bytes(len) as usize];
                stream.read_exact(&mut msg).await?;
                Ok::<_, std::io::Error>(msg)
            };
            let msg = tokio::time::timeout(TIMEOUT, tcp)
                .await
                .map_err(|_| "hết thời gian chờ (TCP)".to_owned())?
                .map_err(|e| e.to_string())?;
            match parse(&msg, sent).ok_or("câu trả lời không hợp lệ")? {
                Parsed::Records(r) => Ok(r),
                Parsed::Truncated => Err("câu trả lời TCP bị cắt".into()),
            }
        }
    }
}

/// Mọi bản ghi TXT của `domain`.
pub async fn txt_records(servers: &[SocketAddr], domain: &str) -> Result<Vec<String>, DnsError> {
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    if !valid_domain(&domain) {
        return Err(DnsError::InvalidName);
    }
    let mut id = [0u8; 2];
    getrandom::fill(&mut id).expect("nguồn ngẫu nhiên");
    let sent = query(u16::from_be_bytes(id), &domain);
    let mut errors = Vec::new();
    for server in servers {
        match ask(*server, &sent).await {
            Ok(records) => return Ok(records),
            Err(e) => errors.push(format!("{server}: {e}")),
        }
    }
    Err(DnsError::Unreachable(errors.join("; ")))
}

/// Máy chủ DNS giả cho test: trả các bản ghi TXT theo tên miền (dùng ở test đơn vị và E2E).
pub mod testing {
    use std::{
        collections::HashMap,
        net::SocketAddr,
        sync::{Arc, Mutex},
    };

    pub type Records = Arc<Mutex<HashMap<String, Vec<String>>>>;

    fn question_name(msg: &[u8]) -> Option<(String, usize)> {
        let mut at = 12;
        let mut labels = Vec::new();
        loop {
            let len = *msg.get(at)? as usize;
            if len == 0 {
                return Some((labels.join("."), at + 5));
            }
            labels
                .push(String::from_utf8_lossy(msg.get(at + 1..at + 1 + len)?).to_ascii_lowercase());
            at += 1 + len;
        }
    }

    pub fn answer(msg: &[u8], records: &Records) -> Option<Vec<u8>> {
        let (name, end) = question_name(msg)?;
        let found = records.lock().ok()?.get(&name).cloned().unwrap_or_default();
        let mut out = msg.get(..end)?.to_vec();
        out[2] = 0x84; // trả lời, có thẩm quyền
        out[3] = 0;
        out[6..8].copy_from_slice(&(found.len() as u16).to_be_bytes());
        for text in found {
            out.extend_from_slice(&[0xc0, 0x0c, 0, 16, 0, 1, 0, 0, 0, 60]);
            let chunks: Vec<&[u8]> = text.as_bytes().chunks(255).collect();
            let length: usize = chunks.iter().map(|c| c.len() + 1).sum();
            out.extend_from_slice(&(length as u16).to_be_bytes());
            for chunk in chunks {
                out.push(chunk.len() as u8);
                out.extend_from_slice(chunk);
            }
        }
        Some(out)
    }

    /// Chạy máy chủ DNS giả ở `addr` (UDP).
    pub async fn serve(addr: &str, records: Records) -> std::io::Result<SocketAddr> {
        let socket = tokio::net::UdpSocket::bind(addr).await?;
        let local = socket.local_addr()?;
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            while let Ok((n, peer)) = socket.recv_from(&mut buf).await {
                if let Some(reply) = answer(&buf[..n], &records) {
                    let _ = socket.send_to(&reply, peer).await;
                }
            }
        });
        Ok(local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn txt_lookup_against_local_server() {
        let records = testing::Records::default();
        records.lock().unwrap().insert(
            "acme.vn".into(),
            vec![
                "v=spf1 -all".into(),
                "feco-domain-verification=abc123".into(),
            ],
        );
        let long = "x".repeat(300);
        records
            .lock()
            .unwrap()
            .insert("long.acme.vn".into(), vec![long.clone()]);
        let server = testing::serve("127.0.0.1:0", records).await.unwrap();

        let found = txt_records(&[server], "Acme.VN.").await.unwrap();
        assert_eq!(found, ["v=spf1 -all", "feco-domain-verification=abc123"]);
        assert_eq!(
            txt_records(&[server], "long.acme.vn").await.unwrap(),
            [long]
        );
        assert!(txt_records(&[server], "khong-co.vn")
            .await
            .unwrap()
            .is_empty());
        // Máy chủ đầu không trả lời thì hỏi máy chủ kế tiếp.
        let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert!(!txt_records(&[dead, server], "acme.vn")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn rejects_bad_names_and_forged_answers() {
        assert!(matches!(
            txt_records(&[], "khong hop le").await,
            Err(DnsError::InvalidName)
        ));
        assert!(!valid_domain("localhost") && !valid_domain("-a.vn") && valid_domain("a-b.com.vn"));
        let sent = query(7, "acme.vn");
        let mut forged = sent.clone();
        forged[2] = 0x84;
        forged[0] ^= 1; // sai mã truy vấn
        assert!(parse(&forged, &sent).is_none());
        let mut other = query(7, "evil.vn");
        other[2] = 0x84;
        assert!(parse(&other, &sent).is_none(), "câu hỏi khác");
    }
}
