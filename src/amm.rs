use solana_network_client::SolanaClient;
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, sync::Arc};

pub const AMM_POOL_DATA_SIZE: usize = 301;

#[derive(Debug, Clone)]
pub struct AmmPoolInfo {
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

impl AmmPoolInfo {
    pub async fn get_base_balance_f64(&self, solana_client: Arc<SolanaClient>) -> Option<f64> {
        let base_balance = solana_client
            .client_arc()
            .clone()
            .get_token_account_balance(&self.pool_base_token_account)
            .await
            .unwrap();
        base_balance.ui_amount
    }

    pub async fn get_quote_balance_f64(&self, solana_client: Arc<SolanaClient>) -> Option<f64> {
        let quote_balance = solana_client
            .client_arc()
            .clone()
            .get_token_account_balance(&self.pool_quote_token_account)
            .await
            .unwrap();
        quote_balance.ui_amount
    }

    pub async fn get_base_balance_string(
        &self,
        solana_client: Arc<SolanaClient>,
    ) -> Option<String> {
        let base_balance = solana_client
            .client_arc()
            .clone()
            .get_token_account_balance(&self.pool_base_token_account)
            .await
            .unwrap();
        Some(base_balance.ui_amount_string)
    }

    pub async fn get_quote_balance_string(&self, solana: Arc<SolanaClient>) -> Option<String> {
        let quote_balance = solana
            .client_arc()
            .clone()
            .get_token_account_balance(&self.pool_quote_token_account)
            .await
            .unwrap();
        Some(quote_balance.ui_amount_string)
    }
}

pub struct Amm {
    pub solana_client: Arc<SolanaClient>,
}

impl Amm {
    pub fn new(solana_client: Arc<SolanaClient>) -> Self {
        Self { solana_client }
    }

    pub async fn get_amm_pool_info(&self, pool_address: &str) -> Result<AmmPoolInfo, String> {
        let account_data = self
            .solana_client
            .client_arc()
            .get_account_data(
                &Pubkey::from_str(pool_address)
                    .map_err(|e| format!("{:?}", e))
                    .unwrap(),
            )
            .await
            .map_err(|e| format!("Failed to get account data: {:?}", e))?;

        Self::parse_amm_data(&account_data)
    }

    /// Parse AMM data - using multiple attempts similar to bond_curve
    pub fn parse_amm_data(data: &[u8]) -> Result<AmmPoolInfo, String> {
        let result = Self::try_parse_with_offset(data, 0) // Standard offset
            .or_else(|| Self::try_parse_with_offset(data, 8)) // May have 8-byte prefix
            .or_else(|| Self::try_parse_with_offset(data, 1)) // Other offset
            .or_else(|| Self::brute_force_parse(data)); // Brute force search
        match result {
            Some(data) => Ok(data),
            None => Err("Unable to parse AMM pool data - data format mismatch".to_string()),
        }
    }

    fn try_parse_with_offset(data: &[u8], offset: usize) -> Option<AmmPoolInfo> {
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
                                                            return Some(AmmPoolInfo {
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
    fn brute_force_parse(data: &[u8]) -> Option<AmmPoolInfo> {
        const MIN_REQUIRED_SIZE: usize = 3 + 32 * 7 + 8; // pool_bump + index + 7 Pubkeys + lp_supply
        for start in 0..data.len().saturating_sub(MIN_REQUIRED_SIZE) {
            if let Some(result) = Self::try_parse_with_offset(data, start) {
                return Some(result);
            }
        }
        None
    }

    /// Another parsing method: based on the actual data structure you provided
    pub fn parse_amm_data_simple(data: &[u8]) -> Result<AmmPoolInfo, String> {
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
    use solana_network_client::Mode;

    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_parse_amm_data_debug() {
        let solana_client = Arc::new(SolanaClient::new(Mode::MAIN).unwrap());
        let amm = Amm::new(solana_client.clone());
        let amm_pool_info = amm
            .get_amm_pool_info("GjK3S2ZgxTVFEkxg43JE8eC1tbztWCseBYyZ8o8sg9f")
            .await
            .unwrap();
        println!(
            "balance 1: {:?}",
            amm_pool_info
                .get_base_balance_f64(solana_client.clone())
                .await
        );
        println!(
            "balance 2: {:?}",
            amm_pool_info
                .get_quote_balance_f64(solana_client.clone())
                .await
        );
        println!("AMM Pool Info: {:?}", amm_pool_info);
    }
}
