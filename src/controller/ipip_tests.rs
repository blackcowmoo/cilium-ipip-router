use crate::ipip::executor::{get_tunnel_name, IpCommand};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tunnel_name_short() {
        let node_name = "node-1";
        let tunnel_name = get_tunnel_name(node_name);
        assert_eq!(tunnel_name, "tun-d50164b9587");
        assert!(tunnel_name.len() <= 15);
    }

    #[test]
    fn test_get_tunnel_name_long() {
        let node_name = "very-long-node-name-that-exceeds-limit";
        let tunnel_name = get_tunnel_name(node_name);
        assert_eq!(tunnel_name, "tun-743e7336877");
        assert!(tunnel_name.len() <= 15);
    }

    #[test]
    fn test_get_tunnel_name_exact_length() {
        let node_name = "exactly10chars";
        let tunnel_name = get_tunnel_name(node_name);
        assert_eq!(tunnel_name, "tun-15754261d2f");
        assert!(tunnel_name.len() <= 15);
    }

    #[test]
    fn test_ip_command_new() {
        let ip_cmd = IpCommand::new();
        assert!(matches!(ip_cmd, IpCommand));
    }
}
