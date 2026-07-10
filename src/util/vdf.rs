use vdf::{PietrzakVDF, PietrzakVDFParams, VDF, VDFParams};

use crate::CONFIG;

const VDF_BITS: u16 = 1024;

fn create_vdf() -> PietrzakVDF {
    PietrzakVDFParams(VDF_BITS).new()
}

pub fn verify_solution(challenge: &[u8], solution: &[u8]) -> bool {
    create_vdf()
        .verify(challenge, CONFIG.internal_config.vdf_difficulty, solution)
        .is_ok()
}

pub fn solve(challenge: &[u8]) -> Result<Vec<u8>, vdf::InvalidIterations> {
    create_vdf().solve(challenge, CONFIG.internal_config.vdf_difficulty)
}

pub fn solution_to_string(solution: &[u8]) -> String {
    solution.iter().map(|n| n.to_string()).collect::<String>()
}
