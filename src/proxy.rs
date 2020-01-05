//! I/O free implementation of HTTP proxy parsing and serializing.

use httparse::Status;
use unwrap::unwrap;

#[derive(Debug, Clone, PartialEq)]
pub struct Data(pub Vec<u8>);

/// State machine for a single HTTP proxy session.
///
/// Transitions:
///
/// - WaitingRequest.on_data() => [WaitingRequest, OnConnectRequest]
/// - OnConnectRequest.on_connected() => ConnectTunnel
///
#[derive(Debug, PartialEq)]
pub enum Session {
    WaitingRequest(WaitingRequest),
    OnConnectRequest(OnConnectRequest),
    /// Connected to target server, hence HTTP tunnel is created.
    ConnectTunnel(ConnectTunnel),
    Failed(Failed),
}

impl Session {
    pub fn waiting_request() -> Self {
        Session::WaitingRequest(WaitingRequest {})
    }

    pub fn on_data(self, data: Data) -> Session {
        match self {
            Session::WaitingRequest(state) => state.on_data(data),
            _ => Session::Failed(Failed {}),
        }
    }
}

// TODO(povilas): add error field
#[derive(Debug, PartialEq)]
pub struct Failed;

#[derive(Debug, PartialEq)]
pub struct WaitingRequest;

impl WaitingRequest {
    pub fn on_data(self, data: Data) -> Session {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = httparse::Request::new(&mut headers);
        // TODO(povilas): transition to error state on error
        match unwrap!(req.parse(&data.0)) {
            Status::Complete(_bytes_parsed) => {
                if let (Some(method), Some(path)) = (req.method, req.path) {
                    if method == "CONNECT" {
                        Session::OnConnectRequest(OnConnectRequest { path: path.to_string()} )
                    } else {
                        // TODO(povilas): how do I add error context? e.g. which method this
                        // actually was
                        Session::Failed(Failed {})
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

#[derive(Debug, PartialEq)]
pub struct OnConnectRequest {
    pub path: String,
}

impl OnConnectRequest {
    pub fn on_connected(self) -> ConnectTunnel {
        ConnectTunnel {}
    }
}

#[derive(Debug, PartialEq)]
pub struct ConnectTunnel;
