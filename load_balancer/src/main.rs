use async_trait::async_trait;
use rand::thread_rng;
use rand::seq::SliceRandom;
use pingora::prelude::*;
use std::sync::{Arc, Mutex};
use pingora_core::services::background::background_service;

use pingora_core::server::Server;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_core::Result;
use pingora_core::listeners::TlsSettings;
pub struct LB(Arc<LoadBalancer<RoundRobin>>);

fn get_host(session: &mut Session) -> String {
    if let Some(host) = session.get_header(http::header::HOST) {
        if let Ok(host_str) = host.to_str() {
            return host_str.to_string();
        }
    }

    if let Some(host) = session.req_header().uri.host() {
        return host.to_string();
    }

    "".to_string()
}

#[async_trait]
impl ProxyHttp for LB {
    type CTX =  Arc<Mutex<usize>>;

    fn new_ctx(&self) -> Self::CTX {
        Arc::new(Mutex::new(0))
    }
    
    async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        // Extract the "Host" header
        let sni = get_host(_session);

        // Just round robin on defaults
        // let upstream = self.0
        //     .select(b"", 256) // hash doesn't matter for round robin
        //     .unwrap();

        println!("Host is: {sni:?}");
        let upstream = match sni.as_str() {
            "service1.example.com" => {
                let service1_ips = vec![
                    "1.1.1.1:443".to_string(),
                    "1.0.0.1:443".to_string(),
                ];
                let mut rng = thread_rng();
                let ip = service1_ips.choose(&mut rng).unwrap();
                ip.clone()
            },
            "service2.example.com" => {
                let service2_ips = vec![
                    "8.8.8.8:443".to_string(),
                    "8.8.4.4:443".to_string(),
                ];
                let mut rng = thread_rng();
                let ip = service2_ips.choose(&mut rng).unwrap();
                ip.clone()
            },
            _ => self.0.select(b"", 256).unwrap().to_string(),
        };

        println!("upstream peer is: {upstream:?}");

        // Set SNI
        // tls should be true if the upstream is https
        let peer = Box::new(HttpPeer::new(upstream, true, sni));
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {

        // Extract the "Host" header and insert it into the upstream request
        upstream_request.insert_header("Host", get_host(_session)).unwrap();
        Ok(())
    }

    async fn request_filter(&self, session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        if !session.req_header().uri.path().starts_with("/api/v1")
        {
            let _ = session.respond_error(403).await;
            // true: tell the proxy that the response is already written
            return Ok(true);
        }
        Ok(false)
    }
}

fn main() {
   let mut my_server = Server::new(None).unwrap();
    my_server.bootstrap();

    // Note that upstreams needs to be declared as `mut` now
    let mut upstreams =
        LoadBalancer::try_from_iter(["35.241.159.148:443", "35.241.159.148:80"]).unwrap();
    
    let hc = TcpHealthCheck::new();
    upstreams.set_health_check(hc);
    upstreams.health_check_frequency = Some(std::time::Duration::from_secs(1));

    let background = background_service("health check", upstreams);
    let upstreams = background.task();
    let mut lb = http_proxy_service(&my_server.configuration, LB(upstreams));
    lb.add_tcp("0.0.0.0:6188");

    let cert_path = format!("{}/keys/server.crt", env!("CARGO_MANIFEST_DIR"));
    let key_path = format!("{}/keys/key.pem", env!("CARGO_MANIFEST_DIR"));

    let mut tls_settings =
        TlsSettings::intermediate(&cert_path, &key_path).unwrap();
    tls_settings.enable_h2();
    lb.add_tls_with_settings("0.0.0.0:6189", None, tls_settings);

    my_server.add_service(background);

    my_server.add_service(lb);
    my_server.run_forever();

}