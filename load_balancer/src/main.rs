use async_trait::async_trait;
use pingora::prelude::*;
use std::sync::Arc;
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
    type CTX = ();

    fn new_ctx(&self) -> () {
        ()
    }

    async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
        let upstream = self.0
            .select(b"", 256) // hash doesn't matter for round robin
            .unwrap();

        println!("upstream peer is: {upstream:?}");

        // Get the RequestHeader

        // Extract the "Host" header
        let sni = get_host(_session);

        // Set SNI
        // tls should be true if the upstream is https
        let peer = Box::new(HttpPeer::new(upstream, false, sni));
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