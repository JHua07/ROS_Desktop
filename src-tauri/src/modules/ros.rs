//! ROS 2 daemon XML-RPC client and graph models.

use crate::modules::{environment::CapturedEnvironment, process::bypass_localhost_proxies};
use quick_xml::{Reader, events::Event};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    env, fmt,
    io::{self, BufRead, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

const DAEMON_BASE_PORT: u16 = 11_511;

#[derive(Clone, Debug, PartialEq)]
pub enum RpcValue {
    Int(i32),
    Bool(bool),
    String(String),
    Array(Vec<RpcValue>),
    Struct(BTreeMap<String, RpcValue>),
    Nil,
}

impl RpcValue {
    fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl Endpoint {
    pub fn for_domain(domain_id: u16) -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: DAEMON_BASE_PORT.saturating_add(domain_id),
            path: "/ros2cli/".into(),
        }
    }

    pub fn from_environment() -> Result<Self, RosError> {
        let domain_id = env::var("ROS_DOMAIN_ID")
            .unwrap_or_else(|_| "0".into())
            .parse::<u16>()
            .map_err(|_| RosError::InvalidDomainId)?;
        Ok(Self::for_domain(domain_id))
    }
}

#[derive(Debug)]
pub enum RosError {
    Io(io::Error),
    InvalidDomainId,
    InvalidResponse(String),
    Http(String),
    Fault(String),
}

impl fmt::Display for RosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "ROS daemon I/O failed: {error}"),
            Self::InvalidDomainId => write!(f, "ROS_DOMAIN_ID must be an unsigned integer"),
            Self::InvalidResponse(message) => write!(f, "invalid ROS daemon response: {message}"),
            Self::Http(message) => write!(f, "ROS daemon HTTP error: {message}"),
            Self::Fault(message) => write!(f, "ROS daemon XML-RPC fault: {message}"),
        }
    }
}

impl std::error::Error for RosError {}
impl From<io::Error> for RosError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RosTopic {
    pub name: String,
    pub types: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RosNode {
    pub name: String,
    pub namespace: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RosService {
    pub name: String,
    pub types: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RosGraphSnapshot {
    pub topics: Vec<RosTopic>,
    pub nodes: Vec<RosNode>,
    pub services: Vec<RosService>,
    pub topic_count: usize,
    pub node_count: usize,
    pub service_count: usize,
}

pub struct XmlRpcClient {
    endpoint: Endpoint,
    timeout: Duration,
}

impl XmlRpcClient {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            timeout: Duration::from_secs(2),
        }
    }

    pub fn call(&self, method: &str, params: &[RpcValue]) -> Result<Vec<RpcValue>, RosError> {
        let body = encode_request(method, params);
        let address = (self.endpoint.host.as_str(), self.endpoint.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| RosError::Http("daemon address did not resolve".into()))?;
        let mut stream = TcpStream::connect_timeout(&address, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.endpoint.path,
            self.endpoint.host,
            self.endpoint.port,
            body.len(),
            body
        );
        stream.write_all(request.as_bytes())?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;
        let separator = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| RosError::Http("missing HTTP header terminator".into()))?;
        let headers = String::from_utf8_lossy(&response[..separator]);
        if !headers.starts_with("HTTP/1.1 200") && !headers.starts_with("HTTP/1.0 200") {
            return Err(RosError::Http(
                headers.lines().next().unwrap_or("unknown status").into(),
            ));
        }
        parse_response(&response[separator + 4..])
    }

    pub fn available(&self) -> bool {
        self.call("system.listMethods", &[]).is_ok()
    }

    pub fn graph_snapshot(&self) -> Result<RosGraphSnapshot, RosError> {
        let topics = parse_topics(self.call("get_topic_names_and_types", &[])?.first())?;
        let nodes = parse_nodes(self.call("get_node_names_and_namespaces", &[])?.first())?;
        let services = parse_services(self.call("get_service_names_and_types", &[])?.first())?;
        Ok(RosGraphSnapshot {
            topic_count: topics.len(),
            node_count: nodes.len(),
            service_count: services.len(),
            topics,
            nodes,
            services,
        })
    }
}

pub fn start_or_reuse_daemon(
    environment: &CapturedEnvironment,
    client: &XmlRpcClient,
) -> Result<(), RosError> {
    if client.available() {
        return Ok(());
    }
    let mut command = environment.command("ros2");
    command.args(["daemon", "start"]);
    bypass_localhost_proxies(&mut command);
    let status = command.status()?;
    if !status.success() {
        return Err(RosError::Http(format!(
            "ros2 daemon start exited with {status}"
        )));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if client.available() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(RosError::Http(
        "ros2 daemon did not become reachable within 5 seconds".into(),
    ))
}

pub fn encode_request(method: &str, params: &[RpcValue]) -> String {
    let parameters = params.iter().map(encode_value).collect::<String>();
    format!(
        "<?xml version=\"1.0\"?><methodCall><methodName>{}</methodName><params>{}</params></methodCall>",
        escape(method),
        parameters
    )
}

fn encode_value(value: &RpcValue) -> String {
    let content = match value {
        RpcValue::Int(value) => format!("<int>{value}</int>"),
        RpcValue::Bool(value) => format!("<boolean>{}</boolean>", if *value { 1 } else { 0 }),
        RpcValue::String(value) => format!("<string>{}</string>", escape(value)),
        RpcValue::Array(values) => format!(
            "<array><data>{}</data></array>",
            values.iter().map(encode_value).collect::<String>()
        ),
        RpcValue::Struct(values) => {
            let members = values
                .iter()
                .map(|(name, value)| {
                    format!(
                        "<member><name>{}</name>{}</member>",
                        escape(name),
                        encode_value(value)
                    )
                })
                .collect::<String>();
            format!("<struct>{members}</struct>")
        }
        RpcValue::Nil => "<nil/>".into(),
    };
    format!("<value>{content}</value>")
}

pub fn parse_response(xml: &[u8]) -> Result<Vec<RpcValue>, RosError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Start(event) if event.name().as_ref() == "params" => {
                return parse_params(&mut reader);
            }
            Event::Start(event) if event.name().as_ref() == "fault" => {
                return Err(RosError::Fault("daemon returned a fault".into()));
            }
            Event::Eof => {
                return Err(RosError::InvalidResponse(
                    "methodResponse has no params".into(),
                ));
            }
            _ => buffer.clear(),
        }
    }
}

fn parse_params<R: BufRead>(reader: &mut Reader<R>) -> Result<Vec<RpcValue>, RosError> {
    let mut values = Vec::new();
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Start(event) if event.name().as_ref() == "value" => {
                values.push(parse_value_contents(reader)?)
            }
            Event::End(event) if event.name().as_ref() == "params" => return Ok(values),
            Event::Eof => return Err(RosError::InvalidResponse("params is not closed".into())),
            _ => buffer.clear(),
        }
    }
}

fn parse_value_contents<R: BufRead>(reader: &mut Reader<R>) -> Result<RpcValue, RosError> {
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Start(event) => {
                let name = event.name().as_ref().to_string();
                return match name.as_str() {
                    "int" | "i4" => {
                        Ok(RpcValue::Int(read_text(reader, &name)?.parse().map_err(
                            |_| RosError::InvalidResponse("invalid integer".into()),
                        )?))
                    }
                    "boolean" => Ok(RpcValue::Bool(read_text(reader, &name)? == "1")),
                    "string" | "name" => Ok(RpcValue::String(read_text(reader, &name)?)),
                    "array" => parse_array(reader),
                    "struct" => parse_struct(reader),
                    other => Err(RosError::InvalidResponse(format!(
                        "unsupported XML-RPC value type {other:?}"
                    ))),
                };
            }
            Event::Empty(event) if event.name().as_ref() == "nil" => return Ok(RpcValue::Nil),
            Event::Text(text) => return Ok(RpcValue::String(text.as_ref().to_owned())),
            Event::End(event) if event.name().as_ref() == "value" => {
                return Ok(RpcValue::String(String::new()));
            }
            Event::Eof => return Err(RosError::InvalidResponse("value is not closed".into())),
            _ => buffer.clear(),
        }
    }
}

fn read_text<R: BufRead>(reader: &mut Reader<R>, end_name: &str) -> Result<String, RosError> {
    let mut text = String::new();
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Text(value) => text.push_str(value.as_ref()),
            Event::End(event) if event.name().as_ref() == end_name => return Ok(text),
            Event::Eof => {
                return Err(RosError::InvalidResponse(
                    "typed value is not closed".into(),
                ));
            }
            _ => buffer.clear(),
        }
    }
}

fn parse_array<R: BufRead>(reader: &mut Reader<R>) -> Result<RpcValue, RosError> {
    let mut values = Vec::new();
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Start(event) if event.name().as_ref() == "value" => {
                values.push(parse_value_contents(reader)?)
            }
            Event::End(event) if event.name().as_ref() == "array" => {
                return Ok(RpcValue::Array(values));
            }
            Event::Eof => return Err(RosError::InvalidResponse("array is not closed".into())),
            _ => buffer.clear(),
        }
    }
}

fn parse_struct<R: BufRead>(reader: &mut Reader<R>) -> Result<RpcValue, RosError> {
    let mut values = BTreeMap::new();
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Start(event) if event.name().as_ref() == "member" => {
                let (name, value) = parse_member(reader)?;
                values.insert(name, value);
            }
            Event::End(event) if event.name().as_ref() == "struct" => {
                return Ok(RpcValue::Struct(values));
            }
            Event::Eof => return Err(RosError::InvalidResponse("struct is not closed".into())),
            _ => buffer.clear(),
        }
    }
}

fn parse_member<R: BufRead>(reader: &mut Reader<R>) -> Result<(String, RpcValue), RosError> {
    let mut name = None;
    let mut value = None;
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| RosError::InvalidResponse(error.to_string()))?
        {
            Event::Start(event) if event.name().as_ref() == "name" => {
                name = Some(read_text(reader, "name")?)
            }
            Event::Start(event) if event.name().as_ref() == "value" => {
                value = Some(parse_value_contents(reader)?)
            }
            Event::End(event) if event.name().as_ref() == "member" => {
                return Ok((
                    name.ok_or_else(|| RosError::InvalidResponse("member has no name".into()))?,
                    value.ok_or_else(|| RosError::InvalidResponse("member has no value".into()))?,
                ));
            }
            Event::Eof => return Err(RosError::InvalidResponse("member is not closed".into())),
            _ => buffer.clear(),
        }
    }
}

fn parse_topics(value: Option<&RpcValue>) -> Result<Vec<RosTopic>, RosError> {
    let values = match value {
        Some(RpcValue::Array(values)) => values,
        _ => {
            return Err(RosError::InvalidResponse(
                "topic result is not an array".into(),
            ));
        }
    };
    Ok(values
        .iter()
        .filter_map(|value| match value {
            RpcValue::Array(pair) if pair.len() >= 2 => Some(RosTopic {
                name: pair[0].as_string()?.to_owned(),
                types: string_array(&pair[1]),
            }),
            _ => None,
        })
        .collect())
}

fn parse_nodes(value: Option<&RpcValue>) -> Result<Vec<RosNode>, RosError> {
    let values = match value {
        Some(RpcValue::Array(values)) => values,
        _ => {
            return Err(RosError::InvalidResponse(
                "node result is not an array".into(),
            ));
        }
    };
    Ok(values
        .iter()
        .filter_map(|value| match value {
            RpcValue::Array(pair) if pair.len() >= 2 => Some(RosNode {
                name: pair[0].as_string()?.to_owned(),
                namespace: pair[1].as_string()?.to_owned(),
            }),
            _ => None,
        })
        .collect())
}

fn parse_services(value: Option<&RpcValue>) -> Result<Vec<RosService>, RosError> {
    let values = match value {
        Some(RpcValue::Array(values)) => values,
        _ => {
            return Err(RosError::InvalidResponse(
                "service result is not an array".into(),
            ));
        }
    };
    Ok(values
        .iter()
        .filter_map(|value| match value {
            RpcValue::Array(pair) if pair.len() >= 2 => Some(RosService {
                name: pair[0].as_string()?.to_owned(),
                types: string_array(&pair[1]),
            }),
            _ => None,
        })
        .collect())
}

fn string_array(value: &RpcValue) -> Vec<String> {
    match value {
        RpcValue::Array(values) => values
            .iter()
            .filter_map(RpcValue::as_string)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{Endpoint, RpcValue, XmlRpcClient, encode_request, parse_response};

    #[test]
    fn endpoint_uses_ros_domain_id_offset() {
        assert_eq!(Endpoint::for_domain(0).port, 11511);
        assert_eq!(Endpoint::for_domain(42).port, 11553);
    }

    #[test]
    fn request_encodes_method_and_string_parameters() {
        let body = encode_request("system.listMethods", &[RpcValue::String("caller".into())]);

        assert!(body.contains("<methodName>system.listMethods</methodName>"));
        assert!(body.contains("<string>caller</string>"));
    }

    #[test]
    fn response_decodes_nested_arrays_and_structs() {
        let xml = r#"<?xml version="1.0"?>
            <methodResponse><params><param><value><array><data>
              <value><string>ready</string></value>
              <value><struct><member><name>count</name><value><int>3</int></value></member></struct></value>
            </data></array></value></param></params></methodResponse>"#;

        let value = parse_response(xml.as_bytes()).expect("valid XML-RPC response");
        let values = match &value[0] {
            RpcValue::Array(values) => values,
            other => panic!("expected array, got {other:?}"),
        };
        assert_eq!(values[0], RpcValue::String("ready".into()));
        let fields = match &values[1] {
            RpcValue::Struct(fields) => fields,
            other => panic!("expected struct, got {other:?}"),
        };
        assert_eq!(fields.get("count"), Some(&RpcValue::Int(3)));
    }

    #[test]
    #[ignore = "requires a running ROS 2 daemon on localhost"]
    fn live_daemon_graph_query_returns_snapshot() {
        let client = XmlRpcClient::new(Endpoint::for_domain(0));
        assert!(client.available());
        let snapshot = client.graph_snapshot().expect("daemon graph query");
        assert_eq!(snapshot.topic_count, snapshot.topics.len());
        assert_eq!(snapshot.node_count, snapshot.nodes.len());
        assert_eq!(snapshot.service_count, snapshot.services.len());
    }
}
