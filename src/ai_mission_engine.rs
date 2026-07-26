use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::player_profile::get_profile_by_owner;
use crate::seasons::{get_current_season, SeasonTheme};
use crate::mission_generator::{get_player_missions};

// ─── Constants ─────────────────────────────────────────────────────────────

/// Minimum base reward for AI-generated missions.
pub const BASE_AI_REWARD: i128 = 200;

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Represents the player's playstyle classification.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum PlayerArchetype {
    Rookie,
    Veteran,
    Explorer,
    Harvester,
    Trader,
    Nomad,
}

/// Dynamic analysis of a player's behaviors, stats, and achievements.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PlayerBehaviourProfile {
    pub total_scans: u32,
    pub essence_earned: i128,
    pub missions_assigned: u32,
    pub missions_completed: u32,
    pub preferred_archetype: PlayerArchetype,
    pub skill_rating: u64,
    pub avg_completion_rate: u32, // Percentage (0-100)
}

/// Adaptive mission template with narrative hooks.
#[derive(Clone, Debug, PartialEq)]
pub struct MissionTemplate {
    pub template_id: u32,
    pub title: &'static str,
    pub description: &'static str,
    pub target_base: u32,
    pub mission_type: Symbol,
}

/// Fully generated personalized mission.
#[derive(Clone, Debug, PartialEq)]
pub struct AiMissionResult {
    pub template_id: u32,
    pub title: String,
    pub description: String,
    pub mission_type: Symbol,
    pub target_count: u32,
    pub reward: i128,
    pub narrative_tier: Symbol, // common, rare, epic, legend
    pub archetype_tag: Symbol,   // rookie, veteran, explorer, harvester, trader, nomad
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum AiMissionKey {
    /// Tracks count of AI missions generated: player -> u32
    GenerationCount(Address),
    /// Prevents repeated template IDs within short window: (player, template_id) -> u64 (expiry timestamp)
    TemplateCooldown(Address, u32),
}

// ─── Player Behavior Classification (ML-inspired) ──────────────────────────

/// Analyze a player's on-chain stats to extract their behavior fingerprint.
pub fn analyze_player_behavior(env: &Env, player: &Address) -> PlayerBehaviourProfile {
    let profile_res = get_profile_by_owner(env, player);
    
    // 1. Gather raw stats (with defaults if profile doesn't exist yet)
    let (scans, essence) = match profile_res {
        Ok(p) => (p.total_scans, p.essence_earned),
        Err(_) => (0u32, 0i128),
    };

    // 2. Scan player mission history
    let player_missions = get_player_missions(env, player.clone());
    let mut missions_assigned = 0u32;
    let mut missions_completed = 0u32;
    for i in 0..player_missions.len() {
        if let Some(m) = player_missions.get(i) {
            missions_assigned += 1;
            if m.completed {
                missions_completed += 1;
            }
        }
    }

    // 3. Compute metrics & skill rating
    let avg_completion_rate = if missions_assigned > 0 {
        (missions_completed * 100) / missions_assigned
    } else {
        75u32 // Optimistic default for new players
    };

    // skill_rating = scans * 10 + essence / 200 + completed_missions * 50
    let skill_rating = (scans as u64 * 10)
        .saturating_add((essence.max(0) as u64) / 200)
        .saturating_add(missions_completed as u64 * 50);

    // 4. Categorize archetype
    let preferred_archetype = if skill_rating < 500 {
        PlayerArchetype::Rookie
    } else if skill_rating > 5000 {
        PlayerArchetype::Veteran
    } else {
        // Evaluate playstyle ratios
        let scans_score = scans * 10;
        let essence_score = (essence.max(0) as u32) / 20;
        let missions_score = missions_completed * 40;

        if scans_score > essence_score * 2 && scans_score > missions_score {
            PlayerArchetype::Explorer
        } else if essence_score > scans_score * 2 && essence_score > missions_score {
            PlayerArchetype::Harvester
        } else if missions_score > scans_score && missions_score > essence_score {
            PlayerArchetype::Trader
        } else {
            PlayerArchetype::Nomad
        }
    };

    PlayerBehaviourProfile {
        total_scans: scans,
        essence_earned: essence,
        missions_assigned,
        missions_completed,
        preferred_archetype,
        skill_rating,
        avg_completion_rate,
    }
}

// ─── Adaptive Difficulty Model ───────────────────────────────────────────────

/// Calculate adaptive difficulty multiplier in basis points (100 bps = 1.0x).
/// Scales difficulty from 1.0x to 5.0x (100 to 500 bps) based on skill and completion rates.
pub fn calculate_adaptive_difficulty(
    env: &Env,
    profile: &PlayerBehaviourProfile,
) -> u32 {
    // Base difficulty scales from 100 bps to 400 bps based on skill rating capped at 10,000
    let skill_cap = 10_000u64;
    let base_diff = 100 + ((profile.skill_rating.min(skill_cap) * 300) / skill_cap) as u32;

    // Completion rate adjustment: rewards consistent completion, relieves struggling players.
    // +100 bps max for 100% completion, -50 bps max for <40% completion.
    let completion_adj = if profile.avg_completion_rate >= 80 {
        ((profile.avg_completion_rate - 80) * 5) // +0 to +100 bps
    } else if profile.avg_completion_rate < 50 {
        // Negative adjustment down to -50 bps
        let deficit = 50 - profile.avg_completion_rate;
        let neg = (deficit * 2).min(50);
        0u32.saturating_sub(neg) // represented as absolute reduction later
    } else {
        0u32
    };

    let mut final_diff = base_diff + completion_adj;
    if profile.avg_completion_rate < 50 {
        let deficit = 50 - profile.avg_completion_rate;
        let neg = (deficit * 2).min(50);
        final_diff = final_diff.saturating_sub(neg);
    }

    // Seasonal boost: check active season theme
    if let Ok(season) = get_current_season(env) {
        match season.config.theme {
            SeasonTheme::VoidTide | SeasonTheme::NovaBurst => {
                final_diff = (final_diff * 110) / 100; // 1.1x multiplier in high-gravity seasons
            }
            _ => {}
        }
    }

    final_diff.clamp(100, 500)
}

// ─── Mission Catalog (54 Templates) ──────────────────────────────────────────

/// Lookup a mission template by archetype index and index within archetype.
pub fn get_mission_template(archetype: &PlayerArchetype, idx: u32) -> MissionTemplate {
    let t_id = (idx % 9) + 1; // 1 to 9 templates per archetype
    
    match archetype {
        PlayerArchetype::Rookie => {
            match t_id {
                1 => MissionTemplate { template_id: 1, title: "Drift Scan Basics", description: "Perform a quick scan of the local dust cloud to map simple gas deposits.", target_base: 3, mission_type: symbol_short!("scan") },
                2 => MissionTemplate { template_id: 2, title: "Essence Scoop Intro", description: "Harvest a small cluster of floating space crystals for essence practice.", target_base: 15, mission_type: symbol_short!("harvest") },
                3 => MissionTemplate { template_id: 3, title: "Wormhole Observation", description: "Locate and register a nearby stable jump coordinate.", target_base: 1, mission_type: symbol_short!("explore") },
                4 => MissionTemplate { template_id: 4, title: "Pawnshop Trade", description: "Complete a simple bartering trade with a wandering merchant ship.", target_base: 1, mission_type: symbol_short!("trade") },
                5 => MissionTemplate { template_id: 5, title: "Target Practice", description: "Scan and resolve 2 low-level stellar anomalies to clear navigation lanes.", target_base: 2, mission_type: symbol_short!("scan") },
                6 => MissionTemplate { template_id: 6, title: "First Flight Check", description: "Scan multiple distinct sectors within the same star system.", target_base: 4, mission_type: symbol_short!("scan") },
                7 => MissionTemplate { template_id: 7, title: "Stardust Collection", description: "Mine loose planetary rings to accumulate raw mineral dust.", target_base: 20, mission_type: symbol_short!("harvest") },
                8 => MissionTemplate { template_id: 8, title: "Safe Passage Probe", description: "Probe local jump lanes for high-energy spatial fractures.", target_base: 1, mission_type: symbol_short!("explore") },
                _ => MissionTemplate { template_id: 9, title: "Supply Line Check", description: "Simulate a cargo delivery by transferring a tiny scrap bounty.", target_base: 1, mission_type: symbol_short!("trade") },
            }
        }
        PlayerArchetype::Explorer => {
            match t_id {
                1 => MissionTemplate { template_id: 10, title: "Ghost Nebula Mapping", description: "Execute a thorough grid scan of the faint Ghost Nebula to catalog layout cells.", target_base: 8, mission_type: symbol_short!("scan") },
                2 => MissionTemplate { template_id: 11, title: "Void Frequency Scan", description: "Track high-frequency energy signatures inside a dark matter pocket.", target_base: 12, mission_type: symbol_short!("scan") },
                3 => MissionTemplate { template_id: 12, title: "Deep Field Survey", description: "Scan uncharted cosmic coordinates to locate hidden layout hotspots.", target_base: 15, mission_type: symbol_short!("scan") },
                4 => MissionTemplate { template_id: 13, title: "System Perimeter Check", description: "Scan all border sectors of a high-radiation gas giant.", target_base: 10, mission_type: symbol_short!("scan") },
                5 => MissionTemplate { template_id: 14, title: "Rogue Wave Profiling", description: "Scan active solar radiation bursts to analyze space weather patterns.", target_base: 9, mission_type: symbol_short!("scan") },
                6 => MissionTemplate { template_id: 15, title: "Stellar Apex Cartography", description: "Complete scans of elite celestial anomalies at the apex of the system.", target_base: 14, mission_type: symbol_short!("scan") },
                7 => MissionTemplate { template_id: 16, title: "Dark Energy Probe", description: "Map out the density fluctuations within a stellar collapse zone.", target_base: 7, mission_type: symbol_short!("scan") },
                8 => MissionTemplate { template_id: 17, title: "Magnetic Ring Study", description: "Execute scans across planetary rings to detect metallic cores.", target_base: 11, mission_type: symbol_short!("scan") },
                _ => MissionTemplate { template_id: 18, title: "Drift Horizon Scanning", description: "Track drift patterns at the extreme boundary coordinates.", target_base: 16, mission_type: symbol_short!("scan") },
            }
        }
        PlayerArchetype::Harvester => {
            match t_id {
                1 => MissionTemplate { template_id: 19, title: "Crystal Vein Extraction", description: "Extract concentrated energy essence from asteroid core fissures.", target_base: 50, mission_type: symbol_short!("harvest") },
                2 => MissionTemplate { template_id: 20, title: "Solar Wind Harvest", description: "Deploy harvesting sweeps to capture solar particles in system fields.", target_base: 75, mission_type: symbol_short!("harvest") },
                3 => MissionTemplate { template_id: 21, title: "Dark Matter Collection", description: "Synthesize high-purity dark matter fractions from background drift.", target_base: 100, mission_type: symbol_short!("harvest") },
                4 => MissionTemplate { template_id: 22, title: "Gas Giant Skimming", description: "Collect gas isotopes from the upper atmosphere of a gas giant.", target_base: 60, mission_type: symbol_short!("harvest") },
                5 => MissionTemplate { template_id: 23, title: "Nebula Core Pumping", description: "Pump raw luminous plasma from active seasonal nebula centers.", target_base: 80, mission_type: symbol_short!("harvest") },
                6 => MissionTemplate { template_id: 24, title: "Asteroid Field Strip", description: "Perform complete mining sweeps across a heavy silicate asteroid belt.", target_base: 120, mission_type: symbol_short!("harvest") },
                7 => MissionTemplate { template_id: 25, title: "Comet Dust Sieving", description: "Sweep frozen comet tails to extract trace sub-zero essence.", target_base: 70, mission_type: symbol_short!("harvest") },
                8 => MissionTemplate { template_id: 26, title: "Supernova Remnant Filter", description: "Filter volatile fusion scraps from old supernova debris clouds.", target_base: 95, mission_type: symbol_short!("harvest") },
                _ => MissionTemplate { template_id: 27, title: "Singularity Dust Pull", description: "Harvest heavy essence particles slipping past event horizons.", target_base: 110, mission_type: symbol_short!("harvest") },
            }
        }
        PlayerArchetype::Trader => {
            match t_id {
                1 => MissionTemplate { template_id: 28, title: "Essence Market Arbitrage", description: "Complete low-risk commodity swaps across alliance border nodes.", target_base: 3, mission_type: symbol_short!("trade") },
                2 => MissionTemplate { template_id: 29, title: "Resource Convoy Escort", description: "Complete supply shipments to outpost markets on behalf of miners.", target_base: 2, mission_type: symbol_short!("trade") },
                3 => MissionTemplate { template_id: 30, title: "Alliance Supply Run", description: "Procure and deliver high-grade industrial parts to guild bays.", target_base: 4, mission_type: symbol_short!("trade") },
                4 => MissionTemplate { template_id: 31, title: "Smuggler Lane Brokerage", description: "Facilitate shadow market exchanges inside dark space outposts.", target_base: 3, mission_type: symbol_short!("trade") },
                5 => MissionTemplate { template_id: 32, title: "Refining Deal Setup", description: "Negotiate raw essence processing contracts with refinery ships.", target_base: 2, mission_type: symbol_short!("trade") },
                6 => MissionTemplate { template_id: 33, title: "Luxury Skin Trade", description: "Verify cosmetic skin cargo transports between regional capital vaults.", target_base: 1, mission_type: symbol_short!("trade") },
                7 => MissionTemplate { template_id: 34, title: "Debris Salvage Auction", description: "Clear and trade ship debris scrap to local scrap yard operators.", target_base: 5, mission_type: symbol_short!("trade") },
                8 => MissionTemplate { template_id: 35, title: "Wormhole Toll Exchange", description: "Purchase navigation clearance certificates for inter-system routes.", target_base: 2, mission_type: symbol_short!("trade") },
                _ => MissionTemplate { template_id: 36, title: "Stellar Treasury Auction", description: "Bid and finalize bonding contracts with system banks.", target_base: 1, mission_type: symbol_short!("trade") },
            }
        }
        PlayerArchetype::Nomad => {
            match t_id {
                1 => MissionTemplate { template_id: 37, title: "Uncharted Sector Probe", description: "Navigate and chart safe passage coordinates in volatile zones.", target_base: 3, mission_type: symbol_short!("explore") },
                2 => MissionTemplate { template_id: 38, title: "Wormhole Cartography", description: "Inspect local gravity anomalies to verify stable wormhole anchors.", target_base: 2, mission_type: symbol_short!("explore") },
                3 => MissionTemplate { template_id: 39, title: "Stellar Relic Hunt", description: "Examine ancient structural debris in dead solar sectors.", target_base: 1, mission_type: symbol_short!("explore") },
                4 => MissionTemplate { template_id: 40, title: "Nebula Core Navigation", description: "Map out flight vector paths through thick, dynamic nebulae.", target_base: 4, mission_type: symbol_short!("explore") },
                5 => MissionTemplate { template_id: 41, title: "Signal Origin Quest", description: "Pinpoint coordinate coordinates of a mysterious transmission.", target_base: 1, mission_type: symbol_short!("explore") },
                6 => MissionTemplate { template_id: 42, title: "Chronos Fracture Scan", description: "Investigate space-time distortions caused by past core collapses.", target_base: 2, mission_type: symbol_short!("explore") },
                7 => MissionTemplate { template_id: 43, title: "Anomaly Field Pathing", description: "Identify 3 distinct pocket zones inside an active dust storm.", target_base: 3, mission_type: symbol_short!("explore") },
                8 => MissionTemplate { template_id: 44, title: "Ancient Beacon Re-link", description: "Transmit synchronization packets to reactivate an old beacon.", target_base: 1, mission_type: symbol_short!("explore") },
                _ => MissionTemplate { template_id: 45, title: "Stellar Nursery Survey", description: "Map high-gravity locations where newborn stars are forming.", target_base: 3, mission_type: symbol_short!("explore") },
            }
        }
        PlayerArchetype::Veteran => {
            match t_id {
                1 => MissionTemplate { template_id: 46, title: "The Lost Nomad's Log", description: "Track down the navigation logs of the legendary explorer vessel 'Voyage-9'.", target_base: 5, mission_type: symbol_short!("explore") },
                2 => MissionTemplate { template_id: 47, title: "Signal from the Drift", description: "Locate and decipher encrypted transmission beacons floating in deep space.", target_base: 3, mission_type: symbol_short!("explore") },
                3 => MissionTemplate { template_id: 48, title: "Echo of the First Star", description: "Investigate cosmic background echoes that date back to galaxy ignition.", target_base: 2, mission_type: symbol_short!("scan") },
                4 => MissionTemplate { template_id: 49, title: "Singularity Core Extract", description: "Harvest raw volatile essence directly from the center of a black hole.", target_base: 250, mission_type: symbol_short!("harvest") },
                5 => MissionTemplate { template_id: 50, title: "Megastructure Survey", description: "Map and analyze the structural layout of a massive ancient space elevator.", target_base: 10, mission_type: symbol_short!("explore") },
                6 => MissionTemplate { template_id: 51, title: "High-Value Cargo Convoy", description: "Coordinate and execute a triple cargo trade deal across hostile sectors.", target_base: 5, mission_type: symbol_short!("trade") },
                7 => MissionTemplate { template_id: 52, title: "Void Rift Stabilization", description: "Scan, harvest, and stabilize a massive high-density anomaly zone.", target_base: 15, mission_type: symbol_short!("scan") },
                8 => MissionTemplate { template_id: 53, title: "Legendary Star Cartography", description: "Map coordinates for the rarest stellar formations in the catalog.", target_base: 8, mission_type: symbol_short!("explore") },
                _ => MissionTemplate { template_id: 54, title: "Eclipse Core Harvesting", description: "Harvest heavy stellar plasma during an eclipse alignment window.", target_base: 200, mission_type: symbol_short!("harvest") },
            }
        }
    }
}

// ─── AI Engine Entrypoint ───────────────────────────────────────────────────

/// Perform AI analysis and generate a procedurally tailored mission for `player`.
pub fn generate_ai_mission_internal(
    env: &Env,
    player: Address,
    seed: u64,
) -> AiMissionResult {
    // 1. Run profile analysis
    let profile = analyze_player_behavior(env, &player);

    // 2. Determine difficulty multiplier (100 - 500 bps)
    let difficulty_bps = calculate_adaptive_difficulty(env, &profile);

    // 3. Map archetype to template indices
    let gen_key = AiMissionKey::GenerationCount(player.clone());
    let gen_count: u32 = env.storage().persistent().get(&gen_key).unwrap_or(0);
    env.storage().persistent().set(&gen_key, &(gen_count + 1));

    // Combine seed with generation counter and player info to make it unique
    let final_seed = seed
        .wrapping_mul(gen_count as u64 + 1)
        .wrapping_add(difficulty_bps as u64);

    let template_idx = (final_seed % 9) as u32;
    let template = get_mission_template(&profile.preferred_archetype, template_idx);

    // 4. Adapt target counts and rewards
    // target_count = base * difficulty_bps / 100 (difficulty scales up target)
    let target_count = (template.target_base * difficulty_bps / 100).max(1);

    // reward = BASE * target * (difficulty_bps / 100)
    let reward = BASE_AI_REWARD
        .saturating_mul(target_count as i128)
        .saturating_mul(difficulty_bps as i128) / 100;

    // 5. Narrative Tier tagging based on difficulty_bps
    let narrative_tier = if difficulty_bps >= 400 {
        symbol_short!("legend")
    } else if difficulty_bps >= 280 {
        symbol_short!("epic")
    } else if difficulty_bps >= 180 {
        symbol_short!("rare")
    } else {
        symbol_short!("common")
    };

    // Archetype tag for client translation
    let archetype_tag = match profile.preferred_archetype {
        PlayerArchetype::Rookie => symbol_short!("rookie"),
        PlayerArchetype::Veteran => symbol_short!("veteran"),
        PlayerArchetype::Explorer => symbol_short!("explorer"),
        PlayerArchetype::Harvester => symbol_short!("harvestr"),
        PlayerArchetype::Trader => symbol_short!("trader"),
        PlayerArchetype::Nomad => symbol_short!("nomad"),
    };

    AiMissionResult {
        template_id: template.template_id,
        title: String::from_str(env, template.title),
        description: String::from_str(env, template.description),
        mission_type: template.mission_type,
        target_count,
        reward,
        narrative_tier,
        archetype_tag,
    }
}
