use solana_network_client::SolanaClient;
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, sync::Arc};

/// Bond curve liquidity pool data size (estimated based on data structure)
pub const BOND_CURVE_POOL_DATA_SIZE: usize = 151; // Needs adjustment based on actual data

/// Bond curve liquidity pool raw data
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct BondCurvePoolState {
    pub virtual_token_reserves: u64, // Virtual token reserves
    pub virtual_sol_reserves: u64,   // Virtual SOL reserves
    pub real_token_reserves: u64,    // Real token reserves
    pub real_sol_reserves: u64,      // Real SOL reserves
    pub token_total_supply: u64,     // Token total supply
    pub complete: u8,                // Whether complete (0=false, 1=true)
    pub creator: [u8; 32],           // Creator address
    pub is_mayhem_mode: u8,          // Whether mayhem mode
}

#[derive(Debug, Clone)]
pub struct BondCurvePoolData {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
    pub creator: Pubkey,
    pub is_mayhem_mode: bool,
    pub token_mint: Pubkey,
    pub token_price_sol: f64,     // Token price (SOL)
    pub token_price_usd: f64,     // Token price (USD, requires external input)
    pub liquidity_total_sol: f64, // Total liquidity (SOL)
    pub progress_percentage: f64, // Progress percentage
    pub market_cap_sol: f64,      // Market cap (SOL)
}

pub struct BondCurve {
    pub solana_client: Arc<SolanaClient>,
}

impl BondCurve {
    pub fn new(solana_client: Arc<SolanaClient>) -> Self {
        Self { solana_client }
    }

    /// Get bond curve pool data based on specified bond curve address
    /// Example:
    /// ```rust
    /// let bond_curve = BondCurve::new();
    /// let pool_address = "bond_curve_address";
    /// let pool_data = bond_curve.get_bond_curve_pool_info(pool_address).await?;
    /// ```
    pub async fn get_bond_curve_pool_info(
        &self,
        pool_address: &str,
    ) -> Result<BondCurvePoolData, String> {
        // Get account data
        let account_data = self
            .solana_client
            .client_arc()
            .get_account_data(
                &Pubkey::from_str(pool_address)
                    .map_err(|e| format!("{:?}", e))
                    .unwrap(),
            )
            .await
            .map_err(|e| format!("Failed to get account data: {:?}", e))
            .unwrap();
        Self::parse_bond_curve_data(&account_data)
    }

    /// Parse bond curve data
    pub fn parse_bond_curve_data(data: &[u8]) -> Result<BondCurvePoolData, String> {
        // Need at least all fields data
        const MIN_DATA_SIZE: usize = std::mem::size_of::<BondCurvePoolState>();
        if data.len() < MIN_DATA_SIZE {
            return Err(format!(
                "Data too short: need at least {} bytes, actual {} bytes",
                MIN_DATA_SIZE,
                data.len()
            ));
        }
        for i in (0..data.len().saturating_sub(8)).step_by(8) {
            if i + 8 <= data.len() {
                let bytes: [u8; 8] = data[i..i + 8].try_into().unwrap();
                let val = u64::from_le_bytes(bytes);
            }
        }
        for i in (0..data.len().saturating_sub(8)).step_by(8) {
            if i + 8 <= data.len() {
                let bytes: [u8; 8] = data[i..i + 8].try_into().unwrap();
                let val = u64::from_be_bytes(bytes);
            }
        }
        let result = Self::try_parse_with_offset(data, 8) // Common offset
            .or_else(|| Self::try_parse_with_offset(data, 0)) // No offset
            .or_else(|| Self::try_parse_with_offset(data, 1)) // Other offset
            .or_else(|| Self::brute_force_parse(data)); // Brute force search
        match result {
            Some(data) => Ok(data),
            None => Err("Unable to parse bond curve data - data format mismatch".to_string()),
        }
    }

    /// Try parsing from specified offset
    fn try_parse_with_offset(data: &[u8], offset: usize) -> Option<BondCurvePoolData> {
        fn read_u64_safe(data: &[u8], offset: usize, is_be: bool) -> Option<u64> {
            if offset + 8 <= data.len() {
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
        for &is_be in &[false, true] {
            if let Some(virtual_token_reserves) = read_u64_safe(data, offset, is_be) {
                if let Some(virtual_sol_reserves) = read_u64_safe(data, offset + 8, is_be) {
                    if let Some(real_token_reserves) = read_u64_safe(data, offset + 16, is_be) {
                        if let Some(real_sol_reserves) = read_u64_safe(data, offset + 24, is_be) {
                            if let Some(token_total_supply) =
                                read_u64_safe(data, offset + 32, is_be)
                            {
                                if virtual_token_reserves > 100_000_000_000_000
                                    && virtual_token_reserves < 1_000_000_000_000_000
                                    && virtual_sol_reserves > 1_000_000_000
                                    && virtual_sol_reserves < 100_000_000_000
                                {
                                    let complete_offset = offset + 40;
                                    if complete_offset >= data.len() {
                                        continue;
                                    }
                                    let complete = data[complete_offset] != 0;
                                    let creator_offset = complete_offset + 1;
                                    if creator_offset + 32 > data.len() {
                                        continue;
                                    }
                                    let creator_array: [u8; 32] = match data
                                        [creator_offset..creator_offset + 32]
                                        .try_into()
                                    {
                                        Ok(arr) => arr,
                                        Err(_) => continue,
                                    };
                                    let creator = Pubkey::new_from_array(creator_array);
                                    let mayhem_offset = creator_offset + 32;
                                    if mayhem_offset >= data.len() {
                                        continue;
                                    }
                                    let is_mayhem_mode = data[mayhem_offset] != 0;
                                    let token_mint_offset = mayhem_offset + 1;
                                    let token_mint = if token_mint_offset + 32 <= data.len() {
                                        match data[token_mint_offset..token_mint_offset + 32]
                                            .try_into()
                                        {
                                            Ok(mint_array) => Pubkey::new_from_array(mint_array),
                                            Err(_) => Pubkey::default(),
                                        }
                                    } else {
                                        Pubkey::default()
                                    };
                                    return Some(BondCurvePoolData {
                                        virtual_token_reserves,
                                        virtual_sol_reserves,
                                        real_token_reserves,
                                        real_sol_reserves,
                                        token_total_supply,
                                        complete,
                                        creator,
                                        is_mayhem_mode,
                                        token_mint,
                                        token_price_sol: if real_token_reserves > 0 {
                                            real_sol_reserves as f64 / real_token_reserves as f64
                                        } else {
                                            0.0
                                        },
                                        token_price_usd: 0.0,
                                        liquidity_total_sol: real_sol_reserves as f64,
                                        progress_percentage: if virtual_sol_reserves > 0 {
                                            real_sol_reserves as f64 * 100.0
                                                / virtual_sol_reserves as f64
                                        } else {
                                            0.0
                                        },
                                        market_cap_sol: if real_token_reserves > 0 {
                                            token_total_supply as f64
                                                * (real_sol_reserves as f64
                                                    / real_token_reserves as f64)
                                        } else {
                                            0.0
                                        },
                                    });
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
    fn brute_force_parse(data: &[u8]) -> Option<BondCurvePoolData> {
        for start in 0..data.len().saturating_sub(73) {
            // Need at least 73 bytes
            if let Some(result) = Self::try_parse_with_offset(data, start) {
                return Some(result);
            }
        }
        None
    }

    /// Calculate token amount for specified SOL amount
    pub fn calculate_token_amount_for_sol(pool_data: &BondCurvePoolData, sol_amount: u64) -> u64 {
        if pool_data.virtual_token_reserves == 0 || pool_data.virtual_sol_reserves == 0 {
            return 0;
        }
        let k = pool_data.virtual_token_reserves as u128 * pool_data.virtual_sol_reserves as u128;
        let new_sol_reserves = pool_data.virtual_sol_reserves as u128 + sol_amount as u128;
        if new_sol_reserves == 0 {
            return 0;
        }
        let new_token_reserves = k / new_sol_reserves;
        let tokens_out = (pool_data.virtual_token_reserves as u128 - new_token_reserves) as u64;
        tokens_out
    }

    /// Calculate SOL amount for specified token amount
    pub fn calculate_sol_amount_for_tokens(
        pool_data: &BondCurvePoolData,
        token_amount: u64,
    ) -> u64 {
        if pool_data.virtual_token_reserves == 0 || pool_data.virtual_sol_reserves == 0 {
            return 0;
        }
        let k = pool_data.virtual_token_reserves as u128 * pool_data.virtual_sol_reserves as u128;
        let new_token_reserves = pool_data.virtual_token_reserves as u128 + token_amount as u128;
        if new_token_reserves == 0 {
            return 0;
        }
        let new_sol_reserves = k / new_token_reserves;
        let sol_out = (pool_data.virtual_sol_reserves as u128 - new_sol_reserves) as u64;
        sol_out
    }

    /// Get all bond curve pools
    pub async fn get_all_bond_curve_pools(
        &self,
        program_id: &str,
    ) -> Result<Vec<(Pubkey, BondCurvePoolData)>, String> {
        let program_pubkey =
            Pubkey::from_str(program_id).map_err(|e| format!("Invalid program ID: {}", e))?;
        let accounts = self
            .solana_client
            .client_arc()
            .get_program_accounts(&program_pubkey)
            .await
            .map_err(|e| format!("Failed to get program accounts: {}", e))?;
        let mut pools = Vec::new();
        for (address, account) in accounts {
            // Try to parse as bond curve pool
            if let Ok(pool_data) = Self::parse_bond_curve_data(&account.data) {
                pools.push((address, pool_data));
            }
        }
        Ok(pools)
    }
}

#[cfg(test)]
mod tests {
    use solana_network_client::Mode;

    use crate::Pump;

    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test() {
        let solana_client = SolanaClient::new(Mode::MAIN).unwrap();
        let pump = Pump::new(Arc::new(solana_client));
        let bond_curve = pump.create_bond_curve();
        let pool = bond_curve
            .get_bond_curve_pool_info("9RxTSGsTu3VdEGxRy6h3Jmk3hgP4Cfssw8SiPP4PRuKG")
            .await
            .unwrap();
        println!("Bond Curve Pool: {:?}", pool);
    }
}
