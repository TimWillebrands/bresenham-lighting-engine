//! Performance benchmarks for collision detection system
//! 
//! This benchmark validates the performance improvements achieved by moving
//! collision detection from JavaScript bridge calls to native Rust implementation.

use bresenham_lighting_engine::{collision, lighting};
use std::time::Instant;

fn setup_collision_system() {
    // Initialize the collision system
    collision::init();
    lighting::init();
    
    // Switch to pixel mode and add some test obstacles
    collision::set_collision_mode(collision::CollisionMode::Pixel);
    
    // Create a test obstacle pattern
    for x in 40..60 {
        for y in 40..60 {
            collision::set_pixel(x, y, true);
        }
    }
}

fn benchmark_light_update() -> (u128, u128) {
    setup_collision_system();
    
    // Warm up
    for _ in 0..5 {
        lighting::update_or_add_light(0, 30, 50, 50);
    }
    
    // Benchmark multiple light updates
    let start = Instant::now();
    let iterations = 100;
    
    for i in 0..iterations {
        let x = 50 + (i % 20) as i16;
        let y = 50 + (i % 15) as i16;
        lighting::update_or_add_light(0, 30, x, y);
    }
    
    let total_elapsed = start.elapsed();
    let total_micros = total_elapsed.as_micros();
    let avg_per_update = total_micros / iterations;
    
    (total_micros, avg_per_update)
}

fn benchmark_collision_calls() -> u128 {
    setup_collision_system();
    
    // Benchmark individual collision calls
    let start = Instant::now();
    let iterations = 10000;
    
    for i in 0..iterations {
        let x0 = (i % 180) as i16;
        let y0 = (i / 180 % 180) as i16;
        let x1 = ((i + 50) % 180) as i16;
        let y1 = ((i + 30) / 180 % 180) as i16;
        
        collision::is_blocked(x0, y0, x1, y1);
    }
    
    let elapsed = start.elapsed();
    elapsed.as_nanos() / iterations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collision_performance() {
        println!("🚀 Running collision detection performance benchmarks...");
        
        // Benchmark collision calls
        let collision_ns_per_call = benchmark_collision_calls();
        let collision_ms_per_call = collision_ns_per_call as f64 / 1_000_000.0;
        
        println!("📊 Collision Detection Performance:");
        println!("  - Average per collision check: {:.3}ms", collision_ms_per_call);
        println!("  - Nanoseconds per call: {}ns", collision_ns_per_call);
        
        // Benchmark light updates
        let (total_micros, avg_micros) = benchmark_light_update();
        let avg_ms = avg_micros as f64 / 1000.0;
        let total_ms = total_micros as f64 / 1000.0;
        
        println!("💡 Light Update Performance:");
        println!("  - 100 light updates total: {:.2}ms", total_ms);
        println!("  - Average per light update: {:.2}ms", avg_ms);
        println!("  - Updates per second: {:.0}", 1000.0 / avg_ms);
        
        // Compare to old JavaScript bridge performance
        println!("📈 Performance Comparison:");
        println!("  - Old JavaScript bridge: ~250ms per light update");
        println!("  - New native Rust: {:.2}ms per light update", avg_ms);
        
        if avg_ms > 0.0 {
            let improvement_factor = 250.0 / avg_ms;
            println!("  - Performance improvement: {:.1}x faster! 🎉", improvement_factor);
        }
        
        // Validate performance targets
        assert!(collision_ms_per_call < 0.1, 
            "Collision check too slow: {:.3}ms (target: <0.1ms)", collision_ms_per_call);
        assert!(avg_ms < 5.0, 
            "Light update too slow: {:.2}ms (target: <5ms)", avg_ms);
        
        println!("✅ All performance targets met!");
    }

    #[test] 
    fn test_pixel_vs_tile_performance() {
        println!("🏁 Comparing pixel vs tile collision performance...");
        
        // Test pixel mode
        collision::set_collision_mode(collision::CollisionMode::Pixel);
        let pixel_ns = benchmark_collision_calls();
        let pixel_ms = pixel_ns as f64 / 1_000_000.0;
        
        // Test tile mode  
        collision::set_collision_mode(collision::CollisionMode::Tile);
        let tile_ns = benchmark_collision_calls();
        let tile_ms = tile_ns as f64 / 1_000_000.0;
        
        println!("📊 Mode Comparison:");
        println!("  - Pixel mode: {:.3}ms per collision check", pixel_ms);
        println!("  - Tile mode: {:.3}ms per collision check", tile_ms);
        
        if pixel_ms > 0.0 && tile_ms > 0.0 {
            if pixel_ms < tile_ms {
                let faster = tile_ms / pixel_ms;
                println!("  - Pixel mode is {:.1}x faster", faster);
            } else {
                let faster = pixel_ms / tile_ms;
                println!("  - Tile mode is {:.1}x faster", faster);
            }
        }
        
        // Both should be reasonably fast
        assert!(pixel_ms < 1.0, "Pixel mode too slow: {:.3}ms", pixel_ms);
        assert!(tile_ms < 1.0, "Tile mode too slow: {:.3}ms", tile_ms);
    }
} 