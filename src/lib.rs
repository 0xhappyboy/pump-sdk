mod bond_curve;
mod global;

use std::sync::Arc;

use solana_network_sdk::{Solana, types::UnifiedError};
use solana_sdk::pubkey::Pubkey;

use crate::{bond_curve::BondCurve, global::PUMP_DOT_FUN_PROGRAM_ID};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct PumpPoolState {
    pub token_mint: [u8; 32],       // Token mint address
    pub bonding_curve: [u8; 32],    // Bonding curve address
    pub virtual_token_reserve: u64, // Virtual token reserve
    pub virtual_sol_reserve: u64,   // Virtual SOL reserve
    pub real_token_reserve: u64,    // Real token reserve
    pub real_sol_reserve: u64,      // Real SOL reserve
    pub total_supply: u64,          // Total supply
    pub current_progress: u64,      // Current progress (0-10000, 10000=100%)
    pub curve_type: u8,             // Curve type
    _padding1: u8,
    pub fee_bps: u16,      // Fee (basis points)
    pub creator: [u8; 32], // Creator
    pub created_at: i64,   // Creation time
    pub last_updated: i64, // Last update time
    pub status: u8,        // Status (0=active, 1=complete, 2=rugged)
    // Explicitly add padding bytes to ensure alignment
    _padding2: [u8; 7],
}

#[derive(Debug, Clone)]
pub struct PumpInfo {
    pub token_mint: Pubkey,
    pub bonding_curve: Pubkey,
    pub virtual_token_reserve: u64,
    pub virtual_sol_reserve: u64,
    pub real_token_reserve: u64,
    pub real_sol_reserve: u64,
    pub total_supply: u64,
    pub current_progress: f64,
    pub curve_type: u8,
    pub fee_bps: u16,
    pub creator: Pubkey,
    pub created_at: i64,
    pub last_updated: i64,
    pub status: u8,
    pub price_sol: f64,
    pub progress_percent: f64,
}

pub struct Pump {
    pub solana: Arc<Solana>,
}

impl Pump {
    /// Create Raydium
    /// Example
    /// ```rust
    /// let sol = Solana::new(solana_network_sdk::types::Mode::MAIN);
    /// let raydium = Raydium::new(Arc::new(sol));
    /// ```
    pub fn new(solana: Arc<Solana>) -> Self {
        Self { solana: solana }
    }

    pub async fn get_pump_info(&self) -> Result<PumpInfo, String> {
        let v = self
            .solana
            .get_account_data(PUMP_DOT_FUN_PROGRAM_ID)
            .await
            .map_err(|e| UnifiedError::Error(format!("{:?}", e)))
            .unwrap();
        let pump_info = Self::parse_pump_info(&v)
            .map_err(|e| UnifiedError::Error(format!("{:?}", e)))
            .unwrap();
        Ok(pump_info)
    }

    /// Parse PUMP program data into PumpInfo struct
    /// data: Account data retrieved from blockchain
    fn parse_pump_info(data: &[u8]) -> Result<PumpInfo, String> {
        const DISCRIMINATOR_LEN: usize = 8;
        if data.len() < DISCRIMINATOR_LEN + 180 {
            return Err(format!(
                "Pump pool data too short. Expected at least {}, got {}",
                DISCRIMINATOR_LEN + 180,
                data.len()
            ));
        }
        let mut offset = DISCRIMINATOR_LEN;
        let read_pubkey = |data: &[u8], o: &mut usize| -> Pubkey {
            let pk = Pubkey::new_from_array(data[*o..*o + 32].try_into().unwrap());
            *o += 32;
            pk
        };
        let read_u64 = |data: &[u8], o: &mut usize| -> u64 {
            let v = u64::from_le_bytes(data[*o..*o + 8].try_into().unwrap());
            *o += 8;
            v
        };
        let read_u8 = |data: &[u8], o: &mut usize| -> u8 {
            let v = data[*o];
            *o += 1;
            v
        };
        let read_u16 = |data: &[u8], o: &mut usize| -> u16 {
            let v = u16::from_le_bytes(data[*o..*o + 2].try_into().unwrap());
            *o += 2;
            v
        };
        let read_i64 = |data: &[u8], o: &mut usize| -> i64 {
            let v = i64::from_le_bytes(data[*o..*o + 8].try_into().unwrap());
            *o += 8;
            v
        };
        let token_mint = read_pubkey(data, &mut offset);
        let bonding_curve = read_pubkey(data, &mut offset);
        let virtual_token_reserve = read_u64(data, &mut offset);
        let virtual_sol_reserve = read_u64(data, &mut offset);
        let real_token_reserve = read_u64(data, &mut offset);
        let real_sol_reserve = read_u64(data, &mut offset);
        let total_supply = read_u64(data, &mut offset);
        let current_progress_raw = read_u64(data, &mut offset);
        let curve_type = read_u8(data, &mut offset);
        offset += 1; // Skip padding byte
        let fee_bps = read_u16(data, &mut offset);
        let creator = read_pubkey(data, &mut offset);
        let created_at = read_i64(data, &mut offset);
        let last_updated = read_i64(data, &mut offset);
        let status = read_u8(data, &mut offset);
        // Calculate price and progress
        let price_sol = if virtual_token_reserve > 0 {
            virtual_sol_reserve as f64 / virtual_token_reserve as f64
        } else {
            0.0
        };
        let current_progress = current_progress_raw as f64 / 10000.0;
        Ok(PumpInfo {
            token_mint,
            bonding_curve,
            virtual_token_reserve,
            virtual_sol_reserve,
            real_token_reserve,
            real_sol_reserve,
            total_supply,
            current_progress,
            curve_type,
            fee_bps,
            creator,
            created_at,
            last_updated,
            status,
            price_sol,
            progress_percent: current_progress * 100.0,
        })
    }

    pub fn create_bond_curve(&self) -> BondCurve {
        BondCurve::new(self.solana.clone())
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use solana_network_sdk::Solana;
    use solana_network_sdk::types::Mode::MAIN;

    use crate::Pump;

    #[tokio::test]
    async fn test() -> Result<(), Box<dyn std::error::Error>> {
        let solana = Solana::new(MAIN).unwrap();
        let pump = Pump::new(Arc::new(solana));
        let pump_info = pump.get_pump_info().await.unwrap();
        println!("Pump Info: {:?}", pump_info);
        Ok(())
    }
}
