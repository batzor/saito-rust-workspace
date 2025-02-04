use std::convert::TryInto;

use bigint::uint::U256;
use log::trace;
use serde::{Deserialize, Serialize};

use crate::common::defs::{SaitoHash, SaitoPublicKey};
use crate::core::data::crypto::hash;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoldenTicket {
    target: SaitoHash,
    random: SaitoHash,
    #[serde_as(as = "[_; 33]")]
    publickey: SaitoPublicKey,
}

impl GoldenTicket {
    #[allow(clippy::new_without_default)]
    pub fn new(target: SaitoHash, random: SaitoHash, publickey: SaitoPublicKey) -> Self {
        return Self {
            target,
            random,
            publickey,
        };
    }

    // TODO - review exact solution generated and mechanism to determine validity
    pub fn generate_solution(
        previous_block_hash: SaitoHash,
        random_bytes: SaitoHash,
        publickey: SaitoPublicKey,
    ) -> SaitoHash {
        let mut vbytes: Vec<u8> = vec![];
        vbytes.extend(&previous_block_hash);
        vbytes.extend(&random_bytes);
        vbytes.extend(&publickey);
        hash(&vbytes)
    }

    // TODO - review exact algorithm in use here
    pub fn is_valid_solution(solution: SaitoHash, difficulty: u64) -> bool {
        let leading_zeroes_required: u64 = difficulty / 16;
        let final_digit: u8 = 15 - ((difficulty % 16) as u8);

        let mut target_string = String::from("");

        //
        // decidely ungainly
        //
        for i in 0..64 {
            if (i as u64) < leading_zeroes_required {
                target_string.push('0');
            } else {
                if (i as u64) == leading_zeroes_required {
                    if final_digit == 0 {
                        target_string.push('0');
                    }
                    if final_digit == 1 {
                        target_string.push('1');
                    }
                    if final_digit == 2 {
                        target_string.push('2');
                    }
                    if final_digit == 3 {
                        target_string.push('3');
                    }
                    if final_digit == 4 {
                        target_string.push('4');
                    }
                    if final_digit == 5 {
                        target_string.push('5');
                    }
                    if final_digit == 6 {
                        target_string.push('6');
                    }
                    if final_digit == 7 {
                        target_string.push('7');
                    }
                    if final_digit == 8 {
                        target_string.push('8');
                    }
                    if final_digit == 9 {
                        target_string.push('9');
                    }
                    if final_digit == 10 {
                        target_string.push('A');
                    }
                    if final_digit == 11 {
                        target_string.push('B');
                    }
                    if final_digit == 12 {
                        target_string.push('C');
                    }
                    if final_digit == 13 {
                        target_string.push('D');
                    }
                    if final_digit == 14 {
                        target_string.push('E');
                    }
                    if final_digit == 15 {
                        target_string.push('F');
                    }
                } else {
                    target_string.push('F');
                }
            }
        }

        let target_hash = hex::decode(target_string).expect("error generating target bytes array");

        let sol = U256::from_big_endian(&solution);
        let tgt = U256::from_big_endian(&target_hash);

        if sol <= tgt {
            return true;
        }

        trace!(
            "GT : solution : {:?} target : {:?}",
            hex::encode(solution),
            hex::encode(target_hash)
        );
        return false;
    }

    pub fn get_target(&self) -> SaitoHash {
        self.target
    }

    pub fn get_random(&self) -> SaitoHash {
        self.random
    }

    pub fn get_publickey(&self) -> SaitoPublicKey {
        self.publickey
    }

    pub fn serialize_for_transaction(&self) -> Vec<u8> {
        let mut vbytes: Vec<u8> = vec![];
        vbytes.extend(&self.target);
        vbytes.extend(&self.random);
        vbytes.extend(&self.publickey);
        vbytes
    }

    pub fn deserialize_for_transaction(bytes: Vec<u8>) -> GoldenTicket {
        let target: SaitoHash = bytes[0..32].try_into().unwrap();
        let random: SaitoHash = bytes[32..64].try_into().unwrap();
        let publickey: SaitoPublicKey = bytes[64..97].try_into().unwrap();
        GoldenTicket::new(target, random, publickey)
    }
}
