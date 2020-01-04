//! I/O free implementation of HTTP proxy parsing and serializing.

use httparse::Status;
use machine::{machine, transitions};
use unwrap::unwrap;

#[derive(Debug, Clone, PartialEq)]
pub struct Data(pub Vec<u8>);

/// A marker to transition when TCP connection with target server is established.
#[derive(Debug, Clone, PartialEq)]
pub struct Connected;

machine!(
    /// State machine for a single HTTP proxy session.
    #[derive(Debug)]
    pub enum Session {
        WaitingRequest,
        OnConnectRequest { pub path: String, },
        /// Connected to target server, hence HTTP tunnel is created.
        ConnectTunnel,
    }
);

transitions!(Session,
    [
        (WaitingRequest, Data) => [WaitingRequest, OnConnectRequest],
        (OnConnectRequest, Connected) => ConnectTunnel
    ]
);

impl WaitingRequest {
    pub fn on_data(self, data: Data) -> Session {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);
        // TODO(povilas): transition to error state on error
        match unwrap!(req.parse(&data.0)) {
            Status::Complete(_bytes_parsed) => {
                if let (Some(method), Some(path)) = (req.method, req.path) {
                    if method == "CONNECT" {
                        Session::on_connect_request(path.to_string())
                    } else {
                        // TODO(povilas): how do I add error context? e.g. which method this
                        // actually was
                        Session::error()
                    }
                } else {
                    Session::waiting_request()
                }
            }
            Status::Partial => {
                panic!("HTTP request was partially parsed - not yet supported!");
            }
        }
    }
}

impl OnConnectRequest {
    pub fn on_connected(self, _: Connected) -> ConnectTunnel {
        ConnectTunnel { }
    }
}
