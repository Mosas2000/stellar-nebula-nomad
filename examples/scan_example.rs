// Example: Ledger-Seeded Nebula Generation
//
// This example demonstrates how the generate_nebula_layout and
// calculate_rarity_tier contract functions work together.
//
// In production, the Soroban contract is invoked via CLI or a dApp frontend.
// The test environment (shown in tests/) uses Env::default() for
// deterministic local testing.

fn main() {
    println!("=== Stellar Nebula Nomad: Nebula Generation Example ===");
    println!();
    println!("Flow:");
    println!("  1. Player provides a 32-byte seed (BytesN<32>)");
    println!("  2. Contract hashes: SHA-256(seed || ledger_sequence || timestamp)");
    println!("  3. XorShift64 PRNG generates a deterministic 16x16 cell grid");
    println!("  4. Each cell gets a CellType and energy value");
    println!("  5. calculate_rarity_tier scores the layout → Rarity enum");
    println!("  6. NebulaScanned event emitted with layout hash");
    println!();
    println!("Cell type distribution:");
    println!("  Empty       30%  |  energy base:  0");
    println!("  Star        15%  |  energy base: 10");
    println!("  Asteroid    15%  |  energy base:  5");
    println!("  GasCloud    15%  |  energy base:  8");
    println!("  StellarDust 10%  |  energy base: 15");
    println!("  DarkMatter   8%  |  energy base: 25");
    println!("  ExoticMatter 5%  |  energy base: 40");
    println!("  Wormhole     2%  |  energy base: 60");
    println!();
    println!("Rarity scoring: rare_cells * 10 + energy_density");
    println!("  Common     0-49   | Uncommon  50-99");
    println!("  Rare     100-149  | Epic     150-199");
    println!("  Legendary  200+");
    println!();
    println!("CLI invocation:");
    println!("  soroban contract invoke --id CONTRACT_ID \\");
    println!("    --fn scan_nebula \\");
    println!("    --arg '{{\"bytes\": \"<64-hex-chars>\"}}' \\");
    println!("    --arg '\"GABC...\"'");
}
