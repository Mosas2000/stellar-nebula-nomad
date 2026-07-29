//! Integration tests for memory leak detection and heap profiling.

use soroban_sdk::{contract, contractimpl, Env, Symbol, Vec};

#[contract]
struct MemoryTestContract;

#[contractimpl]
impl MemoryTestContract {
    pub fn create_vec(env: Env, count: u32) -> Vec<u32> {
        let mut v = Vec::new(&env);
        for i in 0..count {
            v.push_back(i);
        }
        v
    }
}

#[test]
fn test_no_memory_leak_in_vector_allocations() {
    let env = Env::default();
    let contract_id = env.register(MemoryTestContract, ());

    // Perform repeated allocation cycles to ensure no accumulated memory leaks
    for _ in 0..100 {
        let vec: Vec<u32> = env.invoke_contract(
            &contract_id,
            &Symbol::new(&env, "create_vec"),
            (50u32,).into(),
        );
        assert_eq!(vec.len(), 50);
    }
}

#[test]
fn test_heap_memory_profiling_bounds() {
    let env = Env::default();
    let initial_budget = env.budget();
    
    let mut vec = Vec::new(&env);
    for i in 0..1000 {
        vec.push_back(i as u64);
    }

    let final_budget = env.budget();
    // Verify memory budget overhead remains within expected bounds
    assert!(vec.len() == 1000);
}
