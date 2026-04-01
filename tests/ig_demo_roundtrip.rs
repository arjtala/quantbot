//! Integration test: IG demo round-trip.
//!
//! Requires environment variables: IG_API_KEY, IG_USERNAME, IG_PASSWORD
//! Run with: cargo test --test ig_demo_roundtrip -- --ignored

use std::collections::HashMap;

use quantbot::config::{IgConfig, IgEnvironment, InstrumentConfig};
use quantbot::core::portfolio::OrderSide;
use quantbot::execution::ig::engine::IgExecutionEngine;
use quantbot::execution::traits::{ExecutionEngine, OrderRequest, OrderType};

fn demo_config() -> IgConfig {
    let mut instruments = HashMap::new();
    instruments.insert(
        "GBPUSD=X".to_string(),
        InstrumentConfig {
            epic: "CS.D.GBPUSD.TODAY.IP".to_string(),
            min_size: 0.5,
            size_step: 0.1,
            currency_code: None,
            expiry: None,
        },
    );

    IgConfig {
        environment: IgEnvironment::Demo,
        account_id: "Z69YJL".to_string(),
        instruments,
    }
}

#[tokio::test]
#[ignore]
async fn ig_demo_round_trip() {
    let config = demo_config();
    let engine = IgExecutionEngine::new(&config).expect("failed to create engine");

    // 1. Health check (authenticates + reads positions)
    engine
        .health_check()
        .await
        .expect("health check failed — check IG_API_KEY/USERNAME/PASSWORD env vars");
    eprintln!("  [OK] Health check passed");

    // 2. Read initial positions
    let initial_positions = engine.get_positions().await.expect("get_positions failed");
    eprintln!(
        "  [OK] Initial positions: {} open",
        initial_positions.len()
    );

    // 3. Place a minimal BUY order on GBPUSD
    let order = OrderRequest {
        instrument: "GBPUSD=X".to_string(),
        epic: "CS.D.GBPUSD.TODAY.IP".to_string(),
        direction: OrderSide::Buy,
        size: 0.5, // minimum FX size on IG
        order_type: OrderType::Market,
        currency_code: "GBP".to_string(),
        expiry: "DFB".to_string(),
    };

    let acks = engine
        .place_orders(vec![order])
        .await
        .expect("place_orders failed");
    assert_eq!(acks.len(), 1);
    eprintln!(
        "  [OK] Order placed: deal_ref={}, status={:?}",
        acks[0].deal_reference, acks[0].status
    );

    // 4. Verify position exists
    let positions = engine.get_positions().await.expect("get_positions failed");
    let gbp_pos = positions
        .iter()
        .find(|p| p.instrument == "GBPUSD=X");
    assert!(gbp_pos.is_some(), "GBPUSD=X position not found after order");
    eprintln!(
        "  [OK] Position confirmed: size={}, level={:?}",
        gbp_pos.unwrap().size,
        gbp_pos.unwrap().open_level,
    );

    // 5. Flatten all
    engine.flatten_all().await.expect("flatten_all failed");
    eprintln!("  [OK] Flatten completed");

    // 6. Verify flat
    let final_positions = engine.get_positions().await.expect("get_positions failed");
    let gbp_final = final_positions
        .iter()
        .find(|p| p.instrument == "GBPUSD=X");
    assert!(
        gbp_final.is_none(),
        "GBPUSD=X position still exists after flatten"
    );
    eprintln!("  [OK] Verified flat — round-trip complete");
}
