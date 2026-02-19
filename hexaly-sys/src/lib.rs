#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

//! Low-level FFI bindings to Hexaly (LocalSolver)
//!
//! This crate provides unsafe FFI bindings to the Hexaly optimization library.
//! For safe bindings, use the `hexaly` crate instead.

// Include the generated bindings
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_env() {
        unsafe {
            let env = hexaly_create_env();
            assert!(!env.is_null());
            hexaly_delete_env(env);
        }
    }
}
