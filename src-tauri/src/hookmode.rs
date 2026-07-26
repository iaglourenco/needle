use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Modo "hook": o próprio executável do Needle é chamado pelo Claude Code
/// como comando de hook (`needle.exe hook`). Lê o payload JSON do stdin,
/// repassa pro servidor HTTP local do Needle e sai. Sem GUI, sem Tauri, sem
/// dependência de Node — funciona no instalador final sem nada além do
/// próprio app instalado.
///
/// Qualquer falha (app fechado, porta indisponível, timeout) é silenciosa:
/// este caminho nunca deve atrasar ou travar uma sessão do Claude Code.
pub fn run() {
    let mut body = String::new();
    if std::io::stdin().read_to_string(&mut body).is_err() {
        return;
    }
    if body.trim().is_empty() {
        return;
    }

    let Some(port) = read_active_port() else {
        return;
    };

    let _ = post_event(port, &body);
}

fn read_active_port() -> Option<u16> {
    let path = std::env::temp_dir().join("needle").join("port");
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<u16>().ok()
}

fn post_event(port: u16, body: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;

    let request = format!(
        "POST /event HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    );
    stream.write_all(request.as_bytes())?;

    // Drena a resposta pra garantir que o servidor processou antes do
    // processo sair (evita corrida em Windows onde o socket fecha cedo).
    let mut discard = [0u8; 512];
    let _ = stream.read(&mut discard);
    Ok(())
}
