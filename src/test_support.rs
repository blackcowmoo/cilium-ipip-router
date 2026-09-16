//! Isolated network model for route lifecycle tests; never executes host commands.
use crate::ipip::{IpCommandExecutor, Node, RouteConfig};
use k8s_openapi::api::core::v1::{NodeAddress, NodeSpec, NodeStatus};
use std::{cell::RefCell, collections::BTreeMap, io, process::Output};

pub const GROUP: &str = "router.example.com/group";

pub fn config() -> RouteConfig {
    RouteConfig::new(Some(GROUP.into()))
}

pub fn node(name: &str, ip: &str, cidr: Option<&str>, group: Option<&str>) -> Node {
    let mut node = Node::default();
    node.metadata.name = Some(name.into());
    node.metadata.labels = group.map(|group| BTreeMap::from([(GROUP.into(), group.into())]));
    node.spec = Some(NodeSpec {
        pod_cidr: cidr.map(str::to_owned),
        ..Default::default()
    });
    node.status = Some(NodeStatus {
        addresses: Some(vec![NodeAddress {
            type_: "InternalIP".into(),
            address: ip.into(),
        }]),
        ..Default::default()
    });
    node
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Direct(String),
    Tunnel(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tunnel {
    pub local: String,
    pub remote: String,
    pub up: bool,
    pub rp_filter_disabled: bool,
}

pub struct Network {
    pub addresses: String,
    pub fail_on: Option<Vec<String>>,
    pub routes: RefCell<BTreeMap<String, Route>>,
    pub tunnels: RefCell<BTreeMap<String, Tunnel>>,
    pub calls: RefCell<Vec<Vec<String>>>,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            addresses: "2: eth0 inet 192.0.2.1/24 brd 192.0.2.255 scope global eth0\n".into(),
            fail_on: None,
            routes: RefCell::default(),
            tunnels: RefCell::default(),
            calls: RefCell::default(),
        }
    }
}

impl Network {
    fn record(&self, args: &[&str]) -> io::Result<()> {
        let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        self.calls.borrow_mut().push(args.clone());
        if self.fail_on.as_ref() == Some(&args) {
            return Err(io::Error::other("injected command failure"));
        }
        Ok(())
    }

    pub fn route(&self, cidr: &str) -> Option<Route> {
        self.routes.borrow().get(cidr).cloned()
    }
}

impl IpCommandExecutor for Network {
    fn run(&self, args: &[&str]) -> io::Result<Output> {
        self.record(args)?;
        let mut stdout = vec![];
        match args {
            ["-o", "-4", "addr", "show"] => stdout = self.addresses.as_bytes().to_vec(),
            ["link", "show", "dev", name] => {
                if !self.tunnels.borrow().contains_key(*name) {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "no such device"));
                }
            }
            ["tunnel", "add", name, "mode", "ipip", "local", local, "remote", remote] => {
                let mut tunnels = self.tunnels.borrow_mut();
                assert!(!tunnels.contains_key(*name), "duplicate tunnel creation");
                tunnels.insert(
                    (*name).into(),
                    Tunnel {
                        local: (*local).into(),
                        remote: (*remote).into(),
                        up: false,
                        rp_filter_disabled: false,
                    },
                );
            }
            ["link", "set", "dev", name, "up"] => {
                self.tunnels
                    .borrow_mut()
                    .get_mut(*name)
                    .expect("missing tunnel")
                    .up = true;
            }
            ["route", "replace", cidr, "via", ip] => {
                self.routes
                    .borrow_mut()
                    .insert((*cidr).into(), Route::Direct((*ip).into()));
            }
            ["route", "replace", cidr, "dev", name] => {
                let tunnels = self.tunnels.borrow();
                let tunnel = tunnels
                    .get(*name)
                    .expect("route installed before tunnel creation");
                assert!(
                    tunnel.up && tunnel.rp_filter_disabled,
                    "tunnel must be ready before routing"
                );
                self.routes
                    .borrow_mut()
                    .insert((*cidr).into(), Route::Tunnel((*name).into()));
            }
            ["route", "del", cidr] => {
                if self.routes.borrow_mut().remove(*cidr).is_none() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "no such route"));
                }
            }
            ["tunnel", "del", name] => {
                assert!(self.tunnels.borrow_mut().remove(*name).is_some());
                self.routes
                    .borrow_mut()
                    .retain(|_, route| route != &Route::Tunnel((*name).into()));
            }
            _ => panic!("unexpected network command: {args:?}"),
        }
        Ok(Output {
            status: Default::default(),
            stdout,
            stderr: vec![],
        })
    }

    fn disable_reverse_path_filter(&self, interface: &str) -> io::Result<()> {
        self.record(&["rp_filter", interface, "0"])?;
        self.tunnels
            .borrow_mut()
            .get_mut(interface)
            .expect("missing tunnel")
            .rp_filter_disabled = true;
        Ok(())
    }
}
