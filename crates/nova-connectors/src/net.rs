//! Conexão TCP robusta para os drivers nativos.
//!
//! Motivação: `TcpStream::connect(host)` tenta os endereços do DNS **em sequência e sem prazo**.
//! No Linux a glibc pode ordenar primeiro um AAAA inútil (ex.: link-local `fe80::` publicado por
//! engano no DNS) e a conexão fica pendurada ~2 min sem nenhum erro — enquanto Windows/macOS
//! preferem IPv4 ou fazem Happy Eyeballs. Aqui:
//!
//! 1. [`resolve`] descarta endereços que nunca servem para um host remoto vindo do DNS;
//! 2. [`connect_happy_eyeballs`] tenta os restantes em paralelo escalonado (RFC 8305);
//! 3. [`connect_tcp`] aplica o prazo `connectTimeout` e devolve um erro diagnosticado.

use std::collections::VecDeque;
use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use nova_domain::connection::ConnectionConfig;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::{ConnectorError, ConnectorResult, NetErrorKind, diag};

/// Prazo padrão para TCP + TLS + login.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Intervalo entre o início de tentativas sucessivas (RFC 8305 recomenda 250 ms).
pub const ATTEMPT_DELAY: Duration = Duration::from_millis(250);
/// Chave em `ConnectionProfile.options` com o prazo de conexão em segundos.
pub const CONNECT_TIMEOUT_OPTION: &str = "connectTimeout";

/// Prazo de conexão do perfil (`options.connectTimeout`, em segundos; padrão 15 s, máx. 10 min).
pub fn connect_timeout(cfg: &ConnectionConfig) -> Duration {
    cfg.profile
        .options
        .get(CONNECT_TIMEOUT_OPTION)
        .and_then(|v| v.trim().replace(',', ".").parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s > 0.0)
        .map_or(DEFAULT_CONNECT_TIMEOUT, |s| Duration::from_secs_f64(s.min(600.0)))
}

/// Separa `host:porta` (aceita `[::1]:1433` e `::1:1433` como produzido por `tiberius`).
pub fn split_host_port(addr: &str, default_port: u16) -> (String, u16) {
    if let Some(rest) = addr.strip_prefix('[') {
        if let Some((h, tail)) = rest.split_once(']') {
            let port = tail.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(default_port);
            return (h.to_string(), port);
        }
    }
    match addr.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() => match p.parse() {
            Ok(port) => (h.trim_matches(['[', ']']).to_string(), port),
            Err(_) => (addr.to_string(), default_port),
        },
        _ => (addr.to_string(), default_port),
    }
}

fn is_link_local_v6(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

/// Endereço vindo do DNS que nunca deve ser usado para alcançar um servidor remoto.
fn is_unusable_from_dns(addr: &SocketAddr) -> bool {
    match addr.ip() {
        IpAddr::V4(ip) => ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast(),
        // link-local em DNS público é erro de configuração (o Linux ainda o ordena primeiro)
        IpAddr::V6(ip) => ip.is_unspecified() || ip.is_multicast() || is_link_local_v6(&ip),
    }
}

/// Resultado da resolução: endereços utilizáveis (já ordenados) e os descartados.
#[derive(Debug, Default)]
pub struct Resolved {
    pub addrs: Vec<SocketAddr>,
    pub discarded: Vec<SocketAddr>,
}

/// Filtra e ordena endereços (RFC 8305 §4): intercala IPv6/IPv4 começando pela família do
/// primeiro resultado e deixa IPv4 link-local (169.254/16) por último.
pub fn sort_and_filter(raw: impl IntoIterator<Item = SocketAddr>) -> Resolved {
    let mut out = Resolved::default();
    let mut seen = Vec::new();
    let (mut primary, mut secondary, mut last) = (VecDeque::new(), VecDeque::new(), Vec::new());
    let mut first_v6: Option<bool> = None;
    for a in raw {
        if seen.contains(&a) {
            continue;
        }
        seen.push(a);
        if is_unusable_from_dns(&a) {
            out.discarded.push(a);
            continue;
        }
        if matches!(a.ip(), IpAddr::V4(ip) if ip.is_link_local()) {
            last.push(a);
            continue;
        }
        let v6 = a.is_ipv6();
        let first = *first_v6.get_or_insert(v6);
        if v6 == first { primary.push_back(a) } else { secondary.push_back(a) }
    }
    while !primary.is_empty() || !secondary.is_empty() {
        out.addrs.extend(primary.pop_front());
        out.addrs.extend(secondary.pop_front());
    }
    out.addrs.extend(last);
    out
}

/// Resolve `host` (IP literal é usado como está, inclusive link-local com `%scope`).
pub async fn resolve(host: &str, port: u16) -> ConnectorResult<Resolved> {
    let host = host.trim().trim_matches(['[', ']']);
    if host.is_empty() {
        return Err(ConnectorError::Config("informe o host do servidor".into()));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        diag::info(format!("'{host}' é um IP literal: sem consulta DNS"));
        return Ok(Resolved { addrs: vec![SocketAddr::new(ip, port)], discarded: vec![] });
    }
    diag::info(format!("Resolvendo DNS de '{host}'…"));
    let started = std::time::Instant::now();
    let raw = tokio::net::lookup_host((host, port)).await.map_err(|e| {
        diag::error(format!("DNS falhou para '{host}': {e}"));
        ConnectorError::network(
            NetErrorKind::Dns,
            format!("Não foi possível resolver o nome '{host}'."),
            Some(format!("verifique o nome do servidor, o DNS ou a VPN ({e}).")),
        )
    })?;
    let raw: Vec<SocketAddr> = raw.collect();
    diag::info(format!(
        "DNS respondeu em {} ms (ordem do sistema): {}",
        started.elapsed().as_millis(),
        list(&raw)
    ));
    let resolved = sort_and_filter(raw);
    for a in &resolved.discarded {
        diag::warn(format!("Descartado {} (IPv6 link-local/inválido vindo do DNS)", a.ip()));
    }
    if !resolved.addrs.is_empty() {
        diag::info(format!("Ordem de tentativa: {}", list(&resolved.addrs)));
    }
    if resolved.addrs.is_empty() {
        return Err(ConnectorError::network(
            NetErrorKind::Dns,
            format!("O nome '{host}' não possui endereço utilizável."),
            Some(discarded_hint(host, &resolved.discarded).unwrap_or_else(|| "verifique o registro DNS do servidor.".into())),
        ));
    }
    Ok(resolved)
}

fn list(addrs: &[SocketAddr]) -> String {
    if addrs.is_empty() {
        return "(nenhum)".into();
    }
    addrs.iter().map(|a| a.ip().to_string()).collect::<Vec<_>>().join(", ")
}

fn discarded_hint(host: &str, discarded: &[SocketAddr]) -> Option<String> {
    let ll: Vec<String> = discarded.iter().filter(|a| a.is_ipv6()).map(|a| a.ip().to_string()).collect();
    (!ll.is_empty()).then(|| {
        format!(
            "o DNS de '{host}' publica o IPv6 inválido {} (link-local); peça para remover esse registro AAAA.",
            ll.join(", ")
        )
    })
}

/// Tenta os endereços em paralelo escalonado: inicia o próximo a cada `delay` (ou assim que um
/// falhar); o primeiro a conectar vence e os demais são abortados.
pub async fn connect_happy_eyeballs(
    addrs: &[SocketAddr],
    delay: Duration,
) -> Result<(TcpStream, SocketAddr), Vec<(SocketAddr, io::Error)>> {
    let mut pending: VecDeque<SocketAddr> = addrs.iter().copied().collect();
    let mut set: JoinSet<(SocketAddr, io::Result<TcpStream>)> = JoinSet::new();
    let mut errors = Vec::new();
    let started = std::time::Instant::now();
    let spawn = |set: &mut JoinSet<_>, a: SocketAddr| {
        diag::info(format!("TCP → {a}"));
        set.spawn(async move { (a, TcpStream::connect(a).await) });
    };
    loop {
        if set.is_empty() {
            match pending.pop_front() {
                Some(a) => spawn(&mut set, a),
                None => return Err(errors),
            }
        }
        tokio::select! {
            joined = set.join_next() => match joined {
                Some(Ok((addr, Ok(stream)))) => {
                    set.abort_all();
                    diag::ok(format!("TCP conectado em {addr} ({} ms)", started.elapsed().as_millis()));
                    return Ok((stream, addr));
                }
                Some(Ok((addr, Err(e)))) => {
                    diag::warn(format!("TCP {addr} falhou ({} ms): {e}", started.elapsed().as_millis()));
                    errors.push((addr, e));
                    if let Some(a) = pending.pop_front() {
                        spawn(&mut set, a);
                    }
                }
                Some(Err(_)) | None => {}
            },
            _ = tokio::time::sleep(delay), if !pending.is_empty() => {
                if let Some(a) = pending.pop_front() {
                    spawn(&mut set, a);
                }
            }
        }
    }
}

/// Conexão TCP estabelecida por [`connect_tcp`].
#[derive(Debug)]
pub struct Connected {
    pub stream: TcpStream,
    /// Endereço que venceu a corrida.
    pub addr: SocketAddr,
    /// O DNS devolveu endereços inválidos (descartados) — o resolvedor do SO pode travar neles.
    pub dns_had_invalid: bool,
}

/// Resolve + Happy Eyeballs sob o prazo `deadline`, com erro diagnosticado.
pub async fn connect_tcp(host: &str, port: u16, deadline: Instant, timeout: Duration) -> ConnectorResult<Connected> {
    let resolved = tokio::time::timeout_at(deadline, resolve(host, port))
        .await
        .map_err(|_| timeout_error("resolver o nome", host, port, timeout, None))??;
    let hint = discarded_hint(host, &resolved.discarded);
    match tokio::time::timeout_at(deadline, connect_happy_eyeballs(&resolved.addrs, ATTEMPT_DELAY)).await {
        Err(_) => {
            diag::error(format!("Nenhum endereço respondeu dentro do prazo de {} s", timeout.as_secs()));
            Err(timeout_error("abrir a conexão TCP", host, port, timeout, hint))
        }
        Ok(Ok((stream, addr))) => {
            stream.set_nodelay(true)?;
            Ok(Connected { stream, addr, dns_had_invalid: !resolved.discarded.is_empty() })
        }
        Ok(Err(errors)) => Err(diagnose_tcp(host, port, &errors, hint)),
    }
}

/// Erro de prazo estourado em uma etapa da conexão.
pub fn timeout_error(stage: &str, host: &str, port: u16, timeout: Duration, extra: Option<String>) -> ConnectorError {
    let base = format!(
        "verifique se {host}:{port} está acessível desta máquina (firewall, VPN, servidor ligado) \
         ou aumente o \"Timeout de conexão\"."
    );
    ConnectorError::network(
        NetErrorKind::Timeout,
        format!("Tempo esgotado ({}s) ao {stage} com {host}:{port}.", timeout.as_secs_f32().round()),
        Some(extra.map_or(base.clone(), |e| format!("{e} {base}"))),
    )
}

fn diagnose_tcp(host: &str, port: u16, errors: &[(SocketAddr, io::Error)], extra: Option<String>) -> ConnectorError {
    let detail: Vec<String> = errors.iter().map(|(a, e)| format!("{a}: {e}")).collect();
    let refused = errors.iter().any(|(_, e)| e.kind() == io::ErrorKind::ConnectionRefused);
    let (kind, message, hint) = if refused {
        (
            NetErrorKind::Refused,
            format!("Conexão recusada por {host}:{port}."),
            "a porta está fechada: confira a porta, se o serviço está rodando e se aceita conexões TCP/IP.".to_string(),
        )
    } else {
        (
            NetErrorKind::Unreachable,
            format!("Servidor {host}:{port} inalcançável."),
            "verifique rede, VPN e firewall.".to_string(),
        )
    };
    let hint = match extra {
        Some(e) => format!("{e} {hint}"),
        None => hint,
    };
    ConnectorError::network(kind, message, Some(format!("{hint} [{}]", detail.join("; "))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sa(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn descarta_link_local_v6_e_intercala_familias() {
        let r = sort_and_filter([
            sa("[fe80::22e8:d4ff:feb6:750c%2]:1433"),
            sa("[2001:db8::1]:1433"),
            sa("[2001:db8::2]:1433"),
            sa("92.112.179.43:1433"),
            sa("169.254.1.1:1433"),
            sa("92.112.179.43:1433"),
        ]);
        assert_eq!(r.discarded, vec![sa("[fe80::22e8:d4ff:feb6:750c%2]:1433")]);
        assert_eq!(
            r.addrs,
            vec![sa("[2001:db8::1]:1433"), sa("92.112.179.43:1433"), sa("[2001:db8::2]:1433"), sa("169.254.1.1:1433")]
        );
    }

    #[test]
    fn caso_real_so_sobra_ipv4() {
        let r = sort_and_filter([sa("[fe80::22e8:d4ff:feb6:750c%2]:1433"), sa("92.112.179.43:1433")]);
        assert_eq!(r.addrs, vec![sa("92.112.179.43:1433")]);
        assert!(discarded_hint("db", &r.discarded).unwrap().contains("fe80::22e8:d4ff:feb6:750c"));
    }

    #[test]
    fn split_host_port_variantes() {
        assert_eq!(split_host_port("db.local:1433", 1), ("db.local".into(), 1433));
        assert_eq!(split_host_port("[::1]:1500", 1), ("::1".into(), 1500));
        assert_eq!(split_host_port("::1:1500", 1), ("::1".into(), 1500));
        assert_eq!(split_host_port("db.local", 7), ("db.local".into(), 7));
    }

    #[test]
    fn timeout_do_perfil() {
        use nova_domain::connection::ConnectionProfile;
        let mut p: ConnectionProfile = serde_json::from_value(serde_json::json!({"alias": "x", "provider": "mssql"})).unwrap();
        assert_eq!(connect_timeout(&ConnectionConfig::new(p.clone(), None)), DEFAULT_CONNECT_TIMEOUT);
        p.options.insert(CONNECT_TIMEOUT_OPTION.into(), "3".into());
        assert_eq!(connect_timeout(&ConnectionConfig::new(p.clone(), None)), Duration::from_secs(3));
        p.options.insert(CONNECT_TIMEOUT_OPTION.into(), "abc".into());
        assert_eq!(connect_timeout(&ConnectionConfig::new(p, None)), DEFAULT_CONNECT_TIMEOUT);
    }

    #[tokio::test]
    async fn happy_eyeballs_vence_endereco_que_responde() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let good = listener.local_addr().unwrap();
        // 10.255.255.1 não responde (SYN some): sem Happy Eyeballs ficaria pendurado
        let start = std::time::Instant::now();
        let (_s, addr) = connect_happy_eyeballs(&[sa("10.255.255.1:1433"), good], Duration::from_millis(50)).await.unwrap();
        assert_eq!(addr, good);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn prazo_estourado_vira_erro_de_timeout() {
        let timeout = Duration::from_millis(300);
        let err = connect_tcp("10.255.255.1", 1433, Instant::now() + timeout, timeout).await.unwrap_err();
        assert!(matches!(err, ConnectorError::Network { kind: NetErrorKind::Timeout | NetErrorKind::Unreachable, .. }), "{err}");
    }

    #[tokio::test]
    async fn porta_fechada_vira_recusada() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let t = Duration::from_secs(2);
        let err = connect_tcp("127.0.0.1", port, Instant::now() + t, t).await.unwrap_err();
        assert!(matches!(err, ConnectorError::Network { kind: NetErrorKind::Refused, .. }), "{err}");
    }
}
