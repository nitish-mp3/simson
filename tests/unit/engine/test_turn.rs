//! Unit tests for the embedded TURN/STUN server.
//!
//! Covers STUN binding request/response, TURN allocation request/response,
//! credential validation, and rate limiting.

#[cfg(test)]
mod tests {
    use crate::turn::stun::{
        StunMessage, StunMessageType, StunAttribute, StunAttributeType,
        build_binding_request, build_binding_response,
    };
    use crate::turn::allocation::{
        AllocationManager, AllocationRequest, AllocationResponse,
    };
    use crate::turn::auth::{validate_credentials, generate_ephemeral_credentials};
    use crate::turn::ratelimit::RateLimiter;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;

    // ═══════════════════════════════════════════════════════════════════
    //  1. STUN binding request / response
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_stun_binding_request_construction() {
        let req = build_binding_request();
        assert_eq!(req.message_type(), StunMessageType::BindingRequest);
        // Transaction ID is 12 bytes (96 bits) per RFC 5389
        assert_eq!(req.transaction_id().len(), 12);
        // Magic cookie = 0x2112A442
        assert_eq!(req.magic_cookie(), 0x2112A442);
    }

    #[test]
    fn test_stun_binding_response_contains_xor_mapped_address() {
        let client_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            12345,
        );
        let req = build_binding_request();
        let resp = build_binding_response(&req, client_addr);

        assert_eq!(resp.message_type(), StunMessageType::BindingSuccessResponse);
        assert_eq!(
            resp.transaction_id(),
            req.transaction_id(),
            "Response must echo the transaction ID"
        );

        let xma = resp
            .find_attribute(StunAttributeType::XorMappedAddress)
            .expect("Response must include XOR-MAPPED-ADDRESS");
        let decoded_addr = xma.decode_xor_mapped_address(resp.magic_cookie(), resp.transaction_id());
        assert_eq!(decoded_addr, client_addr);
    }

    #[test]
    fn test_stun_binding_request_serialization_round_trip() {
        let req = build_binding_request();
        let bytes = req.to_bytes();
        // STUN header is 20 bytes minimum
        assert!(bytes.len() >= 20);

        let parsed = StunMessage::from_bytes(&bytes).expect("Round-trip must succeed");
        assert_eq!(parsed.message_type(), StunMessageType::BindingRequest);
        assert_eq!(parsed.transaction_id(), req.transaction_id());
    }

    // ═══════════════════════════════════════════════════════════════════
    //  2. TURN allocation request / response
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_allocation_request_creates_relay() {
        let mut mgr = AllocationManager::new(49152, 49200, 10);
        let client_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            54321,
        );
        let req = AllocationRequest {
            client_addr,
            username: "testuser".to_string(),
            requested_transport: 17, // UDP
            lifetime_secs: 600,
        };
        let resp = mgr.allocate(req).expect("Allocation should succeed");
        assert!(resp.relay_port >= 49152 && resp.relay_port <= 49200);
        assert_eq!(resp.lifetime_secs, 600);
        assert!(resp.allocation_id.len() > 0);
    }

    #[test]
    fn test_allocation_respects_max_per_ip() {
        let max_per_ip = 2;
        let mut mgr = AllocationManager::new(49152, 49200, max_per_ip);
        let client_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            54322,
        );

        for i in 0..max_per_ip {
            let req = AllocationRequest {
                client_addr,
                username: format!("user{}", i),
                requested_transport: 17,
                lifetime_secs: 600,
            };
            mgr.allocate(req).expect("Allocation within limit should succeed");
        }

        let req = AllocationRequest {
            client_addr,
            username: "user_over_limit".to_string(),
            requested_transport: 17,
            lifetime_secs: 600,
        };
        let result = mgr.allocate(req);
        assert!(
            result.is_err(),
            "Allocation exceeding max_per_ip must be rejected"
        );
    }

    #[test]
    fn test_allocation_refresh_extends_lifetime() {
        let mut mgr = AllocationManager::new(49152, 49200, 10);
        let client_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3)),
            54323,
        );
        let req = AllocationRequest {
            client_addr,
            username: "refresh_user".to_string(),
            requested_transport: 17,
            lifetime_secs: 300,
        };
        let resp = mgr.allocate(req).expect("Initial allocation must succeed");

        let refreshed = mgr
            .refresh(&resp.allocation_id, 600)
            .expect("Refresh must succeed");
        assert_eq!(refreshed.lifetime_secs, 600);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  3. Credential validation
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_static_credential_validation_success() {
        let result = validate_credentials(
            "testuser",
            "testpassword",
            "homeassistant.local",
            &[("testuser".to_string(), "testpassword".to_string())],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_static_credential_validation_failure() {
        let result = validate_credentials(
            "testuser",
            "wrongpassword",
            "homeassistant.local",
            &[("testuser".to_string(), "testpassword".to_string())],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_ephemeral_credential_generation_and_validation() {
        let shared_secret = "supersecretkey123";
        let ttl_secs = 86400;
        let (username, password) =
            generate_ephemeral_credentials(shared_secret, ttl_secs, "alice");
        // The username format is typically "timestamp:userid"
        assert!(username.contains(":"));

        let is_valid =
            validate_credentials(&username, &password, "homeassistant.local", &[])
                .is_ok()
                || crate::turn::auth::validate_ephemeral(
                    &username,
                    &password,
                    shared_secret,
                )
                .is_ok();
        assert!(is_valid, "Ephemeral credentials must validate");
    }

    // ═══════════════════════════════════════════════════════════════════
    //  4. Rate limiting
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(10); // 10 requests per second
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10));

        for _ in 0..10 {
            assert!(
                limiter.check_rate(ip),
                "Requests within the limit must be allowed"
            );
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(5);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 11));

        for _ in 0..5 {
            assert!(limiter.check_rate(ip));
        }
        assert!(
            !limiter.check_rate(ip),
            "Requests exceeding the per-second limit must be blocked"
        );
    }
}
