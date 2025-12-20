use bytemuck::{Pod, Zeroable};
use solana_network_sdk::Solana;
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, sync::Arc};

pub const AMM_POOL_DATA_SIZE: usize = 301;

#[derive(Debug, Clone)]
pub struct AmmPoolData {
    pub pool_bump: u8,
    pub index: u16,
    pub creator: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub lp_supply: u64,
    pub coin_creator: Pubkey,
    pub base_reserves: u64,
    pub quote_reserves: u64,
    pub base_price: f64,
    pub total_liquidity: f64,
    pub lp_token_price: f64,
}

pub struct Amm {
    pub solana: Arc<Solana>,
}

impl Amm {
    pub fn new(solana: Arc<Solana>) -> Self {
        Self { solana }
    }

    pub async fn get_amm_pool_info(&self, pool_address: &str) -> Result<AmmPoolData, String> {
        let account_data = self
            .solana
            .get_account_data(pool_address)
            .await
            .map_err(|e| format!("Failed to get account data: {:?}", e))?;

        Self::parse_amm_data(&account_data)
    }

    /// Parse AMM data - using multiple attempts similar to bond_curve
    pub fn parse_amm_data(data: &[u8]) -> Result<AmmPoolData, String> {
        let result = Self::try_parse_with_offset(data, 0) // Standard offset
            .or_else(|| Self::try_parse_with_offset(data, 8)) // May have 8-byte prefix
            .or_else(|| Self::try_parse_with_offset(data, 1)) // Other offset
            .or_else(|| Self::brute_force_parse(data)); // Brute force search
        match result {
            Some(data) => Ok(data),
            None => Err("Unable to parse AMM pool data - data format mismatch".to_string()),
        }
    }

    fn try_parse_with_offset(data: &[u8], offset: usize) -> Option<AmmPoolData> {
        fn read_u8_safe(data: &[u8], offset: usize) -> Option<u8> {
            if offset < data.len() {
                Some(data[offset])
            } else {
                None
            }
        }
        fn read_u16_safe(data: &[u8], offset: usize, is_be: bool) -> Option<u16> {
            if offset + 1 < data.len() {
                let bytes: [u8; 2] = data[offset..offset + 2].try_into().ok()?;
                Some(if is_be {
                    u16::from_be_bytes(bytes)
                } else {
                    u16::from_le_bytes(bytes)
                })
            } else {
                None
            }
        }
        fn read_u64_safe(data: &[u8], offset: usize, is_be: bool) -> Option<u64> {
            if offset + 7 < data.len() {
                let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
                Some(if is_be {
                    u64::from_be_bytes(bytes)
                } else {
                    u64::from_le_bytes(bytes)
                })
            } else {
                None
            }
        }
        fn read_pubkey_safe(data: &[u8], offset: usize) -> Option<Pubkey> {
            if offset + 31 < data.len() {
                let bytes: [u8; 32] = data[offset..offset + 32].try_into().ok()?;
                Some(Pubkey::new_from_array(bytes))
            } else {
                None
            }
        }
        for &is_be in &[false, true] {
            if let Some(pool_bump) = read_u8_safe(data, offset) {
                if let Some(index) = read_u16_safe(data, offset + 1, is_be) {
                    if pool_bump == 255 && index == 0 {
                        let creator_offset = offset + 3;
                        if let Some(creator) = read_pubkey_safe(data, creator_offset) {
                            let base_mint_offset = creator_offset + 32;
                            if let Some(base_mint) = read_pubkey_safe(data, base_mint_offset) {
                                let quote_mint_offset = base_mint_offset + 32;
                                if let Some(quote_mint) = read_pubkey_safe(data, quote_mint_offset)
                                {
                                    let lp_mint_offset = quote_mint_offset + 32;
                                    if let Some(lp_mint) = read_pubkey_safe(data, lp_mint_offset) {
                                        let pool_base_offset = lp_mint_offset + 32;
                                        if let Some(pool_base_token_account) =
                                            read_pubkey_safe(data, pool_base_offset)
                                        {
                                            let pool_quote_offset = pool_base_offset + 32;
                                            if let Some(pool_quote_token_account) =
                                                read_pubkey_safe(data, pool_quote_offset)
                                            {
                                                let lp_supply_offset = pool_quote_offset + 32;
                                                if let Some(lp_supply) =
                                                    read_u64_safe(data, lp_supply_offset, is_be)
                                                {
                                                    let coin_creator_offset = lp_supply_offset + 8;
                                                    if let Some(coin_creator) =
                                                        read_pubkey_safe(data, coin_creator_offset)
                                                    {
                                                        if lp_supply > 1_000_000_000
                                                            && lp_supply < 10_000_000_000_000_000
                                                        {
                                                            return Some(AmmPoolData {
                                                                pool_bump,
                                                                index,
                                                                creator,
                                                                base_mint,
                                                                quote_mint,
                                                                lp_mint,
                                                                pool_base_token_account,
                                                                pool_quote_token_account,
                                                                lp_supply,
                                                                coin_creator,
                                                                base_reserves: 0,
                                                                quote_reserves: 0,
                                                                base_price: 0.0,
                                                                total_liquidity: 0.0,
                                                                lp_token_price: 0.0,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Brute force search all possible offsets
    fn brute_force_parse(data: &[u8]) -> Option<AmmPoolData> {
        const MIN_REQUIRED_SIZE: usize = 3 + 32 * 7 + 8; // pool_bump + index + 7 Pubkeys + lp_supply
        for start in 0..data.len().saturating_sub(MIN_REQUIRED_SIZE) {
            if let Some(result) = Self::try_parse_with_offset(data, start) {
                return Some(result);
            }
        }
        None
    }

    /// Another parsing method: based on the actual data structure you provided
    pub fn parse_amm_data_simple(data: &[u8]) -> Result<AmmPoolData, String> {
        if data.len() < 235 {
            return Err(format!(
                "Data too short: need at least 235 bytes, actual {} bytes",
                data.len()
            ));
        }
        for i in 0..data.len().saturating_sub(32) {
            let slice = &data[i..i + 32];
        }
        Err("Need to debug actual data structure".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_parse_amm_data_debug() {
        let solana = Solana::new(solana_network_sdk::types::Mode::MAIN).unwrap();
        let amm = Amm::new(Arc::new(solana));
        // First get raw data
        let account_data = amm
            .solana
            .get_account_data("GjK3S2ZgxTVFEkxg43JE8eC1tbztWCseBYyZ8o8sg9f")
            .await
            .unwrap();
        println!("Data length: {}", account_data.len());
        // Print first 100 bytes for debugging
        println!("First 100 bytes:");
        for (i, byte) in account_data.iter().take(100).enumerate() {
            print!("{:02x} ", byte);
            if (i + 1) % 16 == 0 {
                println!();
            }
        }
        println!();
        // Try to parse
        let result = Amm::parse_amm_data(&account_data);
        println!("Parse result: {:?}", result);
    }
}
