//! Instâncias nomeadas (`HOST\INSTANCIA`) via SQL Server Browser (UDP 1434, protocolo SSRP).
//!
//! Implementado aqui em vez da feature `sql-browser-tokio` do `tiberius` porque aquela tenta os
//! endereços do DNS em sequência, sem filtrar IPv6 link-local e sem o prazo `connectTimeout`
//! — exatamente o cenário que trava no Linux.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::Instant;

use crate::{ConnectorError, ConnectorResult, NetErrorKind, diag, net};

/// Porta UDP do SQL Server Browser.
pub const BROWSER_PORT: u16 = 1434;
/// Espera máxima por resposta de cada endereço.
const PER_ADDR_TIMEOUT: Duration = Duration::from_secs(2);
/// `CLNT_UCAST_INST`: pergunta por uma instância específica.
const CLNT_UCAST_INST: u8 = 0x04;
/// `SVR_RESP`: cabeçalho da resposta.
const SVR_RESP: u8 = 0x05;

/// Descobre a porta TCP da instância nomeada.
pub async fn instance_port(host: &str, instance: &str, deadline: Instant, timeout: Duration) -> ConnectorResult<u16> {
    diag::info(format!("Instância nomeada '{instance}': consultando o SQL Server Browser (UDP {BROWSER_PORT})…"));
    let resolved = tokio::time::timeout_at(deadline, net::resolve(host, BROWSER_PORT))
        .await
        .map_err(|_| net::timeout_error("resolver o nome", host, BROWSER_PORT, timeout, None))??;
    let msg = [&[CLNT_UCAST_INST][..], instance.as_bytes(), &[0]].concat();
    for addr in resolved.addrs {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match ask(addr, &msg, remaining.min(PER_ADDR_TIMEOUT)).await {
            Ok(reply) => {
                return match parse_reply(&reply, instance) {
                    Ok(port) => {
                        diag::ok(format!("SQL Browser em {}: instância '{instance}' na porta TCP {port}", addr.ip()));
                        Ok(port)
                    }
                    Err(e) => {
                        diag::error(format!("SQL Browser em {}: {}", addr.ip(), e));
                        Err(e)
                    }
                };
            }
            Err(e) => diag::warn(format!("SQL Browser em {} não respondeu: {e}", addr.ip())),
        }
    }
    Err(ConnectorError::network(
        NetErrorKind::Unreachable,
        format!("O SQL Server Browser de '{host}' não respondeu sobre a instância '{instance}'."),
        Some(
            "verifique se o serviço \"SQL Server Browser\" está em execução e se a porta UDP 1434 está liberada no \
             firewall; ou informe a porta TCP fixa da instância (HOST,PORTA ou campo Porta)."
                .into(),
        ),
    ))
}

async fn ask(addr: SocketAddr, msg: &[u8], timeout: Duration) -> std::io::Result<Vec<u8>> {
    let local: SocketAddr = if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().expect("endereço fixo");
    let socket = UdpSocket::bind(local).await?;
    socket.send_to(msg, addr).await?;
    let mut buf = vec![0u8; 4096];
    let len = tokio::time::timeout(timeout, socket.recv(&mut buf))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, format!("sem resposta em {} ms", timeout.as_millis())))??;
    buf.truncate(len);
    Ok(buf)
}

/// Resposta: `0x05`, tamanho (u16 LE) e `ServerName;X;InstanceName;Y;IsClustered;No;Version;…;tcp;1433;;`.
fn parse_reply(buf: &[u8], instance: &str) -> ConnectorResult<u16> {
    let invalid = || ConnectorError::Connection("resposta inválida do SQL Server Browser".into());
    if buf.len() < 3 || buf[0] != SVR_RESP {
        return Err(invalid());
    }
    let declared = u16::from_le_bytes([buf[1], buf[2]]) as usize;
    let body = &buf[3..(3 + declared).min(buf.len())];
    let text = String::from_utf8_lossy(body);
    let parts: Vec<&str> = text.split(';').collect();
    let value = |key: &str| {
        parts.chunks(2).find(|kv| kv[0].eq_ignore_ascii_case(key)).and_then(|kv| kv.get(1)).map(|v| v.trim())
    };
    if let Some(name) = value("InstanceName") {
        if !name.eq_ignore_ascii_case(instance) {
            return Err(ConnectorError::network(
                NetErrorKind::Database,
                format!("O servidor respondeu pela instância '{name}', não '{instance}'."),
                Some("confira o nome da instância.".into()),
            ));
        }
    }
    match value("tcp") {
        Some(port) => port.parse().map_err(|_| invalid()),
        None => Err(ConnectorError::network(
            NetErrorKind::Refused,
            format!("A instância '{instance}' não aceita conexões TCP/IP."),
            Some("habilite o protocolo TCP/IP da instância no SQL Server Configuration Manager.".into()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(text: &str) -> Vec<u8> {
        let mut v = vec![SVR_RESP];
        v.extend((text.len() as u16).to_le_bytes());
        v.extend(text.as_bytes());
        v
    }

    #[test]
    fn le_porta_da_resposta() {
        let r = reply("ServerName;SRV01;InstanceName;SQLEXPRESS;IsClustered;No;Version;15.0.2000.5;tcp;49712;;");
        assert_eq!(parse_reply(&r, "sqlexpress").unwrap(), 49712);
    }

    #[test]
    fn sem_tcp_ou_instancia_errada() {
        let r = reply("ServerName;SRV01;InstanceName;SQLEXPRESS;IsClustered;No;Version;15.0;np;\\\\SRV01\\pipe\\sql;;");
        assert!(matches!(parse_reply(&r, "SQLEXPRESS"), Err(ConnectorError::Network { kind: NetErrorKind::Refused, .. })));
        let r = reply("ServerName;SRV01;InstanceName;OUTRA;tcp;1500;;");
        assert!(matches!(parse_reply(&r, "SQLEXPRESS"), Err(ConnectorError::Network { kind: NetErrorKind::Database, .. })));
        assert!(parse_reply(&[1, 2], "x").is_err());
    }

    #[tokio::test]
    async fn consulta_browser_local_simulado() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = [0u8; 256];
            let (n, from) = server.recv_from(&mut buf).await.unwrap();
            assert_eq!(buf[0], CLNT_UCAST_INST);
            assert_eq!(&buf[1..n - 1], b"INST1");
            server.send_to(&reply("ServerName;S;InstanceName;INST1;tcp;50001;;"), from).await.unwrap();
        });
        let msg = [&[CLNT_UCAST_INST][..], b"INST1", &[0]].concat();
        let got = ask(addr, &msg, Duration::from_secs(2)).await.unwrap();
        assert_eq!(parse_reply(&got, "INST1").unwrap(), 50001);
    }
}
