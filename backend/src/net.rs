use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

/// An IP network in CIDR form (`172.16.0.0/12`), or a single address
/// (`10.0.0.1`, treated as `/32` or `/128`). Used for `TRUSTED_PROXIES`: in
/// production the reverse proxy is a container whose address Docker assigns,
/// so trusting a fixed address isn't possible but trusting a range is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpNet {
    addr: IpAddr,
    prefix: u8,
}

impl IpNet {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(net) & mask == u128::from(ip) & mask
            }
            // An IPv4 client reaching a dual-stack listener shows up as an
            // IPv4-mapped IPv6 address; compare it as the IPv4 it is.
            (IpAddr::V4(_), IpAddr::V6(ip)) => ip
                .to_ipv4_mapped()
                .is_some_and(|v4| self.contains(IpAddr::V4(v4))),
            (IpAddr::V6(_), IpAddr::V4(_)) => false,
        }
    }
}

impl From<IpAddr> for IpNet {
    fn from(addr: IpAddr) -> Self {
        let prefix = if addr.is_ipv4() { 32 } else { 128 };
        Self { addr, prefix }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct IpNetParseError(String);

impl fmt::Display for IpNetParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid IP address or CIDR range: {:?}", self.0)
    }
}

impl std::error::Error for IpNetParseError {}

impl FromStr for IpNet {
    type Err = IpNetParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || IpNetParseError(s.to_string());
        let Some((addr, prefix)) = s.split_once('/') else {
            return s.parse::<IpAddr>().map(Self::from).map_err(|_| err());
        };
        let addr: IpAddr = addr.parse().map_err(|_| err())?;
        let prefix: u8 = prefix.parse().map_err(|_| err())?;
        let max = if addr.is_ipv4() { 32 } else { 128 };
        if prefix > max {
            return Err(err());
        }
        Ok(Self { addr, prefix })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    #[test]
    fn bare_address_matches_only_itself() {
        let n = net("10.0.0.1");
        assert!(n.contains(ip("10.0.0.1")));
        assert!(!n.contains(ip("10.0.0.2")));
    }

    #[test]
    fn ipv4_range() {
        let docker = net("172.16.0.0/12");
        assert!(docker.contains(ip("172.16.0.1")));
        assert!(docker.contains(ip("172.19.0.5")));
        assert!(docker.contains(ip("172.31.255.255")));
        assert!(!docker.contains(ip("172.32.0.1")));
        assert!(!docker.contains(ip("203.0.113.9")));
    }

    #[test]
    fn host_bits_in_the_network_address_are_ignored() {
        assert!(net("172.19.4.4/16").contains(ip("172.19.200.1")));
    }

    #[test]
    fn zero_prefix_matches_every_address_of_that_family() {
        let all = net("0.0.0.0/0");
        assert!(all.contains(ip("8.8.8.8")));
        assert!(!all.contains(ip("::1")));
    }

    #[test]
    fn ipv6_range() {
        let n = net("fd00::/8");
        assert!(n.contains(ip("fd12:3456::1")));
        assert!(!n.contains(ip("2001:db8::1")));
    }

    #[test]
    fn ipv4_mapped_ipv6_peer_matches_ipv4_range() {
        assert!(net("172.16.0.0/12").contains(ip("::ffff:172.18.0.3")));
        assert!(!net("172.16.0.0/12").contains(ip("::ffff:8.8.8.8")));
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",
            "not-an-ip",
            "10.0.0.0/",
            "10.0.0.0/33",
            "::/129",
            "10.0.0.0/x",
        ] {
            assert!(bad.parse::<IpNet>().is_err(), "{bad:?} should not parse");
        }
    }
}
