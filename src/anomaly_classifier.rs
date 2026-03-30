use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

#[derive(Clone)]
#[contracttype]
pub enum AnomalyKey {
    Classification(u64),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ClassificationRecord {
    pub anomaly_id: u64,
    pub anomaly_type: Symbol,
    pub confidence: u32,
    pub last_updated: u64,
    pub scan_count: u32,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AnomalyError {
    InsufficientFeatures = 1,
    NotFound = 2,
    Unauthorized = 3,
}

pub fn classify_anomaly(
    env: &Env,
    anomaly_id: u64,
    features: Vec<u32>,
) -> Result<ClassificationRecord, AnomalyError> {
    if features.len() < 3 {
        return Err(AnomalyError::InsufficientFeatures);
    }

    let mut score = 0u32;
    for f in features.iter() {
        score = score.saturating_add(f);
    }

    let anomaly_type = if score > 200 {
        symbol_short!("blackhole")
    } else if score > 120 {
        symbol_short!("wormhole")
    } else {
        symbol_short!("nebula")
    };

    let confidence = core::cmp::min(100, score / 3);
    let record = ClassificationRecord {
        anomaly_id,
        anomaly_type: anomaly_type.clone(),
        confidence,
        last_updated: env.ledger().timestamp(),
        scan_count: 1,
    };

    env.storage()
        .instance()
        .set(&AnomalyKey::Classification(anomaly_id), &record);

    env.events().publish(
        (symbol_short!("anomaly"), symbol_short!("classify")),
        (anomaly_id, anomaly_type, confidence),
    );

    Ok(record)
}

pub fn classify_batch(
    env: &Env,
    records: Vec<(u64, Vec<u32>)>,
) -> Vec<ClassificationRecord> {
    let mut out = Vec::new(env);

    for rec in records.into_iter() {
        let (id, features) = rec;
        if let Ok(classified) = classify_anomaly(env, id, features.clone()) {
            out.push_back(classified);
        }
    }

    out
}

pub fn refine_classification(
    env: &Env,
    anomaly_id: u64,
    new_data: Vec<u32>,
) -> Result<ClassificationRecord, AnomalyError> {
    let mut existing = env
        .storage()
        .instance()
        .get::<AnomalyKey, ClassificationRecord>(&AnomalyKey::Classification(anomaly_id))
        .ok_or(AnomalyError::NotFound)?;

    if new_data.len() < 1 {
        return Err(AnomalyError::InsufficientFeatures);
    }

    let mut score = existing.confidence * existing.scan_count;
    let mut new_score = 0u32;
    for f in new_data.iter() {
        new_score = new_score.saturating_add(f);
    }
    score = score.saturating_add(new_score);
    existing.scan_count = existing.scan_count.saturating_add(1);
    existing.confidence = core::cmp::min(100, score / existing.scan_count);
    existing.last_updated = env.ledger().timestamp();
    existing.anomaly_type = if existing.confidence > 80 {
        symbol_short!("wormhole")
    } else if existing.confidence > 50 {
        symbol_short!("nebula")
    } else {
        symbol_short!("anomaly")
    };

    env.storage()
        .instance()
        .set(&AnomalyKey::Classification(anomaly_id), &existing);

    env.events().publish(
        (symbol_short!("anomaly"), symbol_short!("refined")),
        (anomaly_id, existing.anomaly_type.clone(), existing.confidence),
    );

    Ok(existing)
}

pub fn get_classification(env: &Env, anomaly_id: u64) -> Option<ClassificationRecord> {
    env.storage()
        .instance()
        .get::<AnomalyKey, ClassificationRecord>(&AnomalyKey::Classification(anomaly_id))
}
