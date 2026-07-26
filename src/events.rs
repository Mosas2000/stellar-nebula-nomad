use soroban_sdk::{symbol_short, Address, Env, Symbol};

// ── PvP Combat ───────────────────────────────────────────────────────────────
pub fn topic_pvp_admin_set() -> Symbol {
    symbol_short!("pvp_admin")
}
pub fn topic_pvp_stats_upd() -> Symbol {
    symbol_short!("pvp_stats")
}
pub fn topic_pvp_elo_upd() -> Symbol {
    symbol_short!("pvp_elo")
}
pub fn topic_pvp_last_act() -> Symbol {
    symbol_short!("pvp_lact")
}
pub fn topic_pvp_elo_cfg() -> Symbol {
    symbol_short!("pvp_ecfg")
}
pub fn topic_pvp_rwd_cfg() -> Symbol {
    symbol_short!("pvp_rcfg")
}
pub fn topic_pvp_decline() -> Symbol {
    symbol_short!("pvp_decl")
}
pub fn topic_pvp_mq_leave() -> Symbol {
    symbol_short!("pvp_mqlv")
}
pub fn topic_pvp_spec_rm() -> Symbol {
    symbol_short!("pvp_sprm")
}

// ── Escrow Trader ────────────────────────────────────────────────────────────
pub fn topic_escrow_confirm() -> Symbol {
    symbol_short!("esc_conf")
}

// ── Nomad Bonding ────────────────────────────────────────────────────────────
pub fn topic_essence_accrue() -> Symbol {
    symbol_short!("ess_accr")
}

// ── Ship NFT ─────────────────────────────────────────────────────────────────
pub fn topic_ship_repair() -> Symbol {
    symbol_short!("ship_rep")
}
pub fn topic_ship_damage() -> Symbol {
    symbol_short!("ship_dmg")
}

// ── Mission Generator ────────────────────────────────────────────────────────
pub fn topic_mission_prog() -> Symbol {
    symbol_short!("mis_prog")
}

// ── Bug Bounty ───────────────────────────────────────────────────────────────
pub fn topic_bounty_submit() -> Symbol {
    symbol_short!("bug_sub")
}
pub fn topic_bounty_fund() -> Symbol {
    symbol_short!("bug_fund")
}
pub fn topic_burst_bounty() -> Symbol {
    symbol_short!("bug_brs")
}
pub fn topic_bounty_pause() -> Symbol {
    symbol_short!("bug_paus")
}
pub fn topic_bounty_comv() -> Symbol {
    symbol_short!("bug_comv")
}

// ── Bounty Board ─────────────────────────────────────────────────────────────
pub fn topic_bounty_approve() -> Symbol {
    symbol_short!("bty_appr")
}
pub fn topic_bounty_dispute() -> Symbol {
    symbol_short!("bty_disp")
}
pub fn topic_bounty_init() -> Symbol {
    symbol_short!("bty_init")
}
pub fn topic_bounty_expiry() -> Symbol {
    symbol_short!("bty_expr")
}

// ── Player Profile ───────────────────────────────────────────────────────────
pub fn topic_achieve_unlock() -> Symbol {
    symbol_short!("ach_ulck")
}

// ── Referral System ──────────────────────────────────────────────────────────
pub fn topic_ref_first_scan() -> Symbol {
    symbol_short!("ref_fscn")
}

// ── Content Tools ────────────────────────────────────────────────────────────
pub fn topic_content_play() -> Symbol {
    symbol_short!("cnt_play")
}
pub fn topic_content_unlist() -> Symbol {
    symbol_short!("cnt_unlt")
}
pub fn topic_content_admin() -> Symbol {
    symbol_short!("cnt_admn")
}
pub fn topic_content_rev_cfg() -> Symbol {
    symbol_short!("cnt_rev")
}

// ── Portal Registry ──────────────────────────────────────────────────────────
pub fn topic_portal_refresh() -> Symbol {
    symbol_short!("prt_refr")
}
pub fn topic_portal_init() -> Symbol {
    symbol_short!("prt_init")
}

// ── Prize Distributor ────────────────────────────────────────────────────────
pub fn topic_prize_snap() -> Symbol {
    symbol_short!("prz_snap")
}
pub fn topic_prize_init() -> Symbol {
    symbol_short!("prz_init")
}

// ── State Snapshot ───────────────────────────────────────────────────────────
pub fn topic_snap_reset() -> Symbol {
    symbol_short!("snp_rset")
}

// ── Gas Sponsor ──────────────────────────────────────────────────────────────
pub fn topic_sponsor_verify() -> Symbol {
    symbol_short!("spn_ver")
}

// ── Achievements ─────────────────────────────────────────────────────────────
pub fn topic_achieve_lb_inc() -> Symbol {
    symbol_short!("ach_lbi")
}
pub fn topic_achieve_try() -> Symbol {
    symbol_short!("ach_try")
}

// ── Leaderboards ─────────────────────────────────────────────────────────────
pub fn topic_lb_admin() -> Symbol {
    symbol_short!("lb_admin")
}
pub fn topic_lb_guild_upd() -> Symbol {
    symbol_short!("lb_gupd")
}
pub fn topic_lb_reg_upd() -> Symbol {
    symbol_short!("lb_rupd")
}
pub fn topic_lb_ach_upd() -> Symbol {
    symbol_short!("lb_aupd")
}
pub fn topic_lb_guild_set() -> Symbol {
    symbol_short!("lb_gset")
}

// ── Seasons ──────────────────────────────────────────────────────────────────
pub fn topic_season_part() -> Symbol {
    symbol_short!("snp_part")
}

// ── Battle Pass ──────────────────────────────────────────────────────────────
pub fn topic_bp_init() -> Symbol {
    symbol_short!("bp_init")
}

// ── Config / Admin (cross-module) ────────────────────────────────────────────
pub fn topic_meta_gateway() -> Symbol {
    symbol_short!("meta_gw")
}
pub fn topic_meta_autopin() -> Symbol {
    symbol_short!("meta_ap")
}
pub fn topic_energy_regen() -> Symbol {
    symbol_short!("eng_reg")
}
pub fn topic_rate_limit() -> Symbol {
    symbol_short!("rt_cfg")
}
pub fn topic_vault_lockdur() -> Symbol {
    symbol_short!("vlt_ldu")
}
pub fn topic_recipe_set() -> Symbol {
    symbol_short!("rcp_set")
}
pub fn topic_recipe_unlock() -> Symbol {
    symbol_short!("rcp_ulck")
}
pub fn topic_frac_cfg() -> Symbol {
    symbol_short!("frac_cfg")
}
pub fn topic_ver_automig() -> Symbol {
    symbol_short!("ver_amg")
}
pub fn topic_export_optin() -> Symbol {
    symbol_short!("exp_opt")
}
pub fn topic_export_cmp() -> Symbol {
    symbol_short!("exp_cmp")
}
pub fn topic_cfg_sign_add() -> Symbol {
    symbol_short!("cfg_sga")
}
pub fn topic_cfg_sign_rm() -> Symbol {
    symbol_short!("cfg_sgr")
}

// ── Init Events ──────────────────────────────────────────────────────────────
pub fn topic_init_roles() -> Symbol {
    symbol_short!("init_rls")
}
pub fn topic_init_bounty() -> Symbol {
    symbol_short!("init_bty")
}
pub fn topic_init_bug() -> Symbol {
    symbol_short!("init_bug")
}
pub fn topic_init_cfg() -> Symbol {
    symbol_short!("init_cfg")
}
pub fn topic_init_ver() -> Symbol {
    symbol_short!("init_ver")
}
pub fn topic_init_fleet() -> Symbol {
    symbol_short!("init_flt")
}
pub fn topic_init_refund() -> Symbol {
    symbol_short!("init_ref")
}
pub fn topic_init_nav() -> Symbol {
    symbol_short!("init_nav")
}
pub fn topic_init_onboard() -> Symbol {
    symbol_short!("init_onb")
}
pub fn topic_init_portal() -> Symbol {
    symbol_short!("init_prt")
}
pub fn topic_init_prize() -> Symbol {
    symbol_short!("init_prz")
}
pub fn topic_init_recycle() -> Symbol {
    symbol_short!("init_rec")
}
pub fn topic_init_upgrade() -> Symbol {
    symbol_short!("init_upg")
}
pub fn topic_init_econtrol() -> Symbol {
    symbol_short!("init_emc")
}
pub fn topic_init_event_fw() -> Symbol {
    symbol_short!("init_efw")
}
pub fn topic_init_scheduler() -> Symbol {
    symbol_short!("init_sch")
}

// ── Batch / Indirect ─────────────────────────────────────────────────────────
pub fn topic_batch_cfg() -> Symbol {
    symbol_short!("cfg_batch")
}
pub fn topic_nav_conn() -> Symbol {
    symbol_short!("nav_conn")
}
pub fn topic_nav_conn_batch() -> Symbol {
    symbol_short!("nav_cnbt")
}
pub fn topic_env_mod() -> Symbol {
    symbol_short!("env_mod")
}
pub fn topic_anom_batch() -> Symbol {
    symbol_short!("anm_batc")
}
pub fn topic_econ_track() -> Symbol {
    symbol_short!("eco_trk")
}
pub fn topic_comp_gas() -> Symbol {
    symbol_short!("cmp_gas")
}
pub fn topic_health_rec() -> Symbol {
    symbol_short!("hlth_rec")
}
pub fn topic_health_batch() -> Symbol {
    symbol_short!("hlth_bat")
}
pub fn topic_batch_clr() -> Symbol {
    symbol_short!("bat_clr")
}
pub fn topic_fleet_ship() -> Symbol {
    symbol_short!("flt_shp")
}
pub fn topic_nav_graph() -> Symbol {
    symbol_short!("nav_grp")
}
pub fn topic_tutorial_path() -> Symbol {
    symbol_short!("tut_pth")
}
pub fn topic_emc_unpause() -> Symbol {
    symbol_short!("emc_ups")
}
pub fn topic_sched_reset() -> Symbol {
    symbol_short!("sch_rst")
}
pub fn topic_sched_part() -> Symbol {
    symbol_short!("sch_prt")
}
pub fn topic_regen_apply() -> Symbol {
    symbol_short!("reg_apl")
}
pub fn topic_data_batch() -> Symbol {
    symbol_short!("dat_bat")
}
pub fn topic_ship_mint_rec() -> Symbol {
    symbol_short!("ship_mir")
}

// ── Helper: publish a standard event ─────────────────────────────────────────
pub fn emit(env: &Env, topic: Symbol, data: impl soroban_sdk::IntoVal<Env, soroban_sdk::Val>) {
    env.events().publish((symbol_short!("evt"), topic), data);
}
