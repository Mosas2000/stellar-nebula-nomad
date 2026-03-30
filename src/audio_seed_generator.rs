use soroban_sdk::{contracterror, contracttype, symbol_short, Env, BytesN, Vec};

// Audio generation constants
pub const INSTRUMENT_PRESETS: u32 = 12;
pub const MAX_LAYERS_PER_NEBULA: u32 = 8;
pub const SEED_SIZE: u32 = 32;

#[derive(Clone)]
#[contracttype]
pub enum AudioKey {
    NebulaSeed(u64),           // nebula_id -> BytesN<32>
    InstrumentPreset(u32),     // preset_id -> InstrumentParams
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AudioError {
    InvalidLayer = 1,
    InvalidNebulaId = 2,
    SeedNotFound = 3,
    InvalidPreset = 4,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct InstrumentParams {
    pub preset_id: u32,
    pub frequency: u32,      // Base frequency in Hz
    pub amplitude: u32,      // Volume level 0-100
    pub waveform: u32,       // 0=sine, 1=square, 2=triangle, 3=sawtooth
    pub attack: u32,         // Attack time in ms
    pub decay: u32,          // Decay time in ms
    pub sustain: u32,        // Sustain level 0-100
    pub release: u32,        // Release time in ms
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct MusicSeed {
    pub nebula_id: u64,
    pub seed: BytesN<32>,
    pub generated_at: u64,
}

/// Initialize default instrument presets
pub fn initialize_presets(env: &Env) {
    // Preset 0: Deep Bass
    env.storage().persistent().set(&AudioKey::InstrumentPreset(0), &InstrumentParams {
        preset_id: 0,
        frequency: 55,
        amplitude: 70,
        waveform: 0,
        attack: 100,
        decay: 200,
        sustain: 60,
        release: 300,
    });
    
    // Preset 1: Ambient Pad
    env.storage().persistent().set(&AudioKey::InstrumentPreset(1), &InstrumentParams {
        preset_id: 1,
        frequency: 220,
        amplitude: 50,
        waveform: 0,
        attack: 500,
        decay: 300,
        sustain: 70,
        release: 800,
    });
    
    // Preset 2: Cosmic Lead
    env.storage().persistent().set(&AudioKey::InstrumentPreset(2), &InstrumentParams {
        preset_id: 2,
        frequency: 440,
        amplitude: 80,
        waveform: 1,
        attack: 50,
        decay: 100,
        sustain: 50,
        release: 200,
    });
    
    // Preset 3: Ethereal Strings
    env.storage().persistent().set(&AudioKey::InstrumentPreset(3), &InstrumentParams {
        preset_id: 3,
        frequency: 330,
        amplitude: 60,
        waveform: 2,
        attack: 300,
        decay: 400,
        sustain: 80,
        release: 600,
    });
    
    // Preset 4: Pulsing Synth
    env.storage().persistent().set(&AudioKey::InstrumentPreset(4), &InstrumentParams {
        preset_id: 4,
        frequency: 110,
        amplitude: 65,
        waveform: 1,
        attack: 20,
        decay: 50,
        sustain: 40,
        release: 100,
    });
    
    // Preset 5: Nebula Drone
    env.storage().persistent().set(&AudioKey::InstrumentPreset(5), &InstrumentParams {
        preset_id: 5,
        frequency: 82,
        amplitude: 55,
        waveform: 3,
        attack: 1000,
        decay: 500,
        sustain: 90,
        release: 1500,
    });
    
    // Preset 6: Stellar Chime
    env.storage().persistent().set(&AudioKey::InstrumentPreset(6), &InstrumentParams {
        preset_id: 6,
        frequency: 880,
        amplitude: 75,
        waveform: 0,
        attack: 10,
        decay: 150,
        sustain: 30,
        release: 400,
    });
    
    // Preset 7: Dark Matter Rumble
    env.storage().persistent().set(&AudioKey::InstrumentPreset(7), &InstrumentParams {
        preset_id: 7,
        frequency: 40,
        amplitude: 85,
        waveform: 3,
        attack: 200,
        decay: 300,
        sustain: 70,
        release: 500,
    });
    
    // Preset 8: Plasma Whisper
    env.storage().persistent().set(&AudioKey::InstrumentPreset(8), &InstrumentParams {
        preset_id: 8,
        frequency: 1760,
        amplitude: 45,
        waveform: 2,
        attack: 150,
        decay: 200,
        sustain: 50,
        release: 350,
    });
    
    // Preset 9: Ion Storm
    env.storage().persistent().set(&AudioKey::InstrumentPreset(9), &InstrumentParams {
        preset_id: 9,
        frequency: 660,
        amplitude: 70,
        waveform: 1,
        attack: 30,
        decay: 80,
        sustain: 60,
        release: 150,
    });
    
    // Preset 10: Crystal Resonance
    env.storage().persistent().set(&AudioKey::InstrumentPreset(10), &InstrumentParams {
        preset_id: 10,
        frequency: 1320,
        amplitude: 65,
        waveform: 0,
        attack: 100,
        decay: 250,
        sustain: 75,
        release: 600,
    });
    
    // Preset 11: Void Echo
    env.storage().persistent().set(&AudioKey::InstrumentPreset(11), &InstrumentParams {
        preset_id: 11,
        frequency: 165,
        amplitude: 50,
        waveform: 2,
        attack: 400,
        decay: 600,
        sustain: 40,
        release: 1000,
    });
}

/// Generate deterministic music seed from nebula state
pub fn generate_music_seed(env: &Env, nebula_id: u64) -> Result<MusicSeed, AudioError> {
    if nebula_id == 0 {
        return Err(AudioError::InvalidNebulaId);
    }
    
    // Create deterministic seed from ledger hash and nebula ID
    let ledger_seq = env.ledger().sequence();
    let timestamp = env.ledger().timestamp();
    
    let mut seed_data = [0u8; 32];
    
    // Mix nebula_id, ledger sequence, and timestamp
    let nebula_bytes = nebula_id.to_be_bytes();
    let seq_bytes = ledger_seq.to_be_bytes();
    let time_bytes = timestamp.to_be_bytes();
    
    // XOR-based deterministic mixing
    for i in 0..8 {
        seed_data[i] = nebula_bytes[i];
        seed_data[i + 8] = seq_bytes[i % 4];
        seed_data[i + 16] = time_bytes[i];
        seed_data[i + 24] = nebula_bytes[i] ^ seq_bytes[i % 4] ^ time_bytes[i];
    }
    
    let seed = BytesN::from_array(env, &seed_data);
    let current_time = env.ledger().timestamp();
    
    let music_seed = MusicSeed {
        nebula_id,
        seed: seed.clone(),
        generated_at: current_time,
    };
    
    // Store seed for nebula
    env.storage()
        .persistent()
        .set(&AudioKey::NebulaSeed(nebula_id), &seed);
    
    // Emit event
    env.events().publish(
        (symbol_short!("audio"), symbol_short!("seed")),
        (nebula_id, seed),
    );
    
    Ok(music_seed)
}

/// Get instrument layer parameters for frontend rendering
pub fn get_instrument_layer(
    env: &Env,
    seed: BytesN<32>,
    layer: u32,
) -> Result<InstrumentParams, AudioError> {
    if layer >= MAX_LAYERS_PER_NEBULA {
        return Err(AudioError::InvalidLayer);
    }
    
    // Derive preset from seed and layer
    let seed_byte = seed.get(layer % SEED_SIZE).unwrap_or(0);
    let preset_id = (seed_byte as u32) % INSTRUMENT_PRESETS;
    
    // Get base preset
    let mut params = env
        .storage()
        .persistent()
        .get::<AudioKey, InstrumentParams>(&AudioKey::InstrumentPreset(preset_id))
        .ok_or(AudioError::InvalidPreset)?;
    
    // Modulate parameters based on seed for variation
    let mod_byte1 = seed.get((layer + 8) % SEED_SIZE).unwrap_or(128);
    let mod_byte2 = seed.get((layer + 16) % SEED_SIZE).unwrap_or(128);
    
    // Apply subtle variations (±20%)
    let freq_mod = ((mod_byte1 as i32 - 128) * params.frequency as i32) / 640;
    params.frequency = ((params.frequency as i32 + freq_mod).max(20).min(2000)) as u32;
    
    let amp_mod = ((mod_byte2 as i32 - 128) * params.amplitude as i32) / 640;
    params.amplitude = ((params.amplitude as i32 + amp_mod).max(10).min(100)) as u32;
    
    Ok(params)
}

/// Get all 8 layers for a nebula instantly
pub fn get_all_layers(env: &Env, nebula_id: u64) -> Result<Vec<InstrumentParams>, AudioError> {
    let seed = env
        .storage()
        .persistent()
        .get::<AudioKey, BytesN<32>>(&AudioKey::NebulaSeed(nebula_id))
        .ok_or(AudioError::SeedNotFound)?;
    
    let mut layers = Vec::new(env);
    
    for layer in 0..MAX_LAYERS_PER_NEBULA {
        let params = get_instrument_layer(env, seed.clone(), layer)?;
        layers.push_back(params);
    }
    
    Ok(layers)
}

/// Get stored seed for a nebula
pub fn get_nebula_seed(env: &Env, nebula_id: u64) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get::<AudioKey, BytesN<32>>(&AudioKey::NebulaSeed(nebula_id))
}

/// Get instrument preset by ID
pub fn get_preset(env: &Env, preset_id: u32) -> Result<InstrumentParams, AudioError> {
    if preset_id >= INSTRUMENT_PRESETS {
        return Err(AudioError::InvalidPreset);
    }
    
    env.storage()
        .persistent()
        .get::<AudioKey, InstrumentParams>(&AudioKey::InstrumentPreset(preset_id))
        .ok_or(AudioError::InvalidPreset)
}
