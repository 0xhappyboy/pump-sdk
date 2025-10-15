use futures_util::StreamExt;
use solana_client::{
    nonblocking::{pubsub_client::PubsubClient, rpc_client::RpcClient},
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
    rpc_response::Response,
};
use solana_commitment_config::CommitmentConfig;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::global::{
    PUMP_DOT_FUN_PROGRAM_ID, SOLANA_OFFICIAL_MAIN_NET_URL, WSS_SOLANA_OFFICIAL_MAIN_NET_URL,
};

#[derive(Debug, Clone)]
pub enum TradeLogInstructionType {
    Trade,
    Swap,
    Buy,
    Sell,
    None,
}

#[derive(Debug, Clone)]
pub enum LiquidityLogInstructionType {
    Liquidity,
    Migrate,
    Raydium,
    None,
}

/// trade event
#[derive(Debug, Clone)]
pub struct TradeEvent {
    pub signature: String,
    pub mint: String,
    pub trade_event_type: TradeLogInstructionType,
}

/// liquidity event
#[derive(Debug, Clone)]
pub struct LiquidityEvent {
    pub signature: String,
    pub mint: String,
    pub liquidity_event_type: LiquidityLogInstructionType,
}

/// Pump.fun Listener
///
/// # Example
/// ``` rust
/// use pump_sdk::listener::Listener;
/// #[tokio::main]
/// pub async fn main() {
///     let listener = Listener::new(500, 500);
///     let _ = listener
///         .start(
///             |_event| {
///                 // handle trade event
///             },
///             |_event| {
///                 // handle liquidity event
///             },
///         )
///         .await;
///     let _ = tokio::signal::ctrl_c().await;
/// }
/// ```
pub struct Listener {
    rpc_url: String,
    ws_url: String,
    client: Arc<RpcClient>,
    trade_sender: broadcast::Sender<TradeEvent>,
    liquidity_sender: broadcast::Sender<LiquidityEvent>,
}

impl Listener {
    pub fn new(trade_channel: usize, liquidity_channel: usize) -> Self {
        let client = Arc::new(RpcClient::new(SOLANA_OFFICIAL_MAIN_NET_URL.to_string()));
        let (trade_channel, _) = broadcast::channel(trade_channel);
        let (liquidity_channel, _) = broadcast::channel(liquidity_channel);
        Self {
            rpc_url: SOLANA_OFFICIAL_MAIN_NET_URL.to_string(),
            ws_url: WSS_SOLANA_OFFICIAL_MAIN_NET_URL.to_string(),
            client: client,
            trade_sender: trade_channel,
            liquidity_sender: liquidity_channel,
        }
    }
    pub fn from_url(
        rpc_url: String,
        ws_url: String,
        trade_channel: usize,
        liquidity_channel: usize,
    ) -> Self {
        let client = Arc::new(RpcClient::new(rpc_url.to_string()));
        let (trade_channel, _) = broadcast::channel(trade_channel);
        let (liquidity_channel, _) = broadcast::channel(liquidity_channel);
        Self {
            rpc_url: rpc_url.to_string(),
            ws_url: ws_url.to_string(),
            client: client,
            trade_sender: trade_channel,
            liquidity_sender: liquidity_channel,
        }
    }
    pub fn from_client(
        client: Arc<RpcClient>,
        ws_url: String,
        trade_channel: usize,
        liquidity_channel: usize,
    ) -> Self {
        let (trade_channel, _) = broadcast::channel(trade_channel);
        let (liquidity_channel, _) = broadcast::channel(liquidity_channel);
        Self {
            rpc_url: SOLANA_OFFICIAL_MAIN_NET_URL.to_string(),
            ws_url: ws_url.to_string(),
            client: client,
            trade_sender: trade_channel,
            liquidity_sender: liquidity_channel,
        }
    }
    /// start listener
    pub async fn start<T, L>(&self, trade_callback: T, liquidity_callback: L) -> Result<(), String>
    where
        T: Fn(TradeEvent) + 'static + Send,
        L: Fn(LiquidityEvent) + 'static + Send,
    {
        // try connection
        if let Err(e) = self.try_connection().await {
            return Err(format!("Try Connection error: {:?}", e));
        }
        // start event handle thread
        self.start_handle_event(trade_callback, liquidity_callback)
            .await;
        // start pump.fun listen
        self.start_program_listener(PUMP_DOT_FUN_PROGRAM_ID.to_string())
            .await?;
        Ok(())
    }
    /// start listen pump.fun program
    async fn start_program_listener(&self, program_id: String) -> Result<(), String> {
        let trade_sender = self.trade_sender.clone();
        let liquidity_sender = self.liquidity_sender.clone();
        let ws_url = self.ws_url.clone();
        tokio::spawn(async move {
            loop {
                let _ =
                    Self::listen_to_program(&ws_url, &program_id, &trade_sender, &liquidity_sender)
                        .await;
            }
        });
        Ok(())
    }
    /// start handle event thread
    /// get the events that have been successfully monitored from the channel.
    async fn start_handle_event<T, L>(&self, trade_callback: T, liquidity_callback: L)
    where
        T: Fn(TradeEvent) + 'static + Send,
        L: Fn(LiquidityEvent) + 'static + Send,
    {
        let mut trade_receiver = self.trade_sender.subscribe();
        let mut liquidity_receiver = self.liquidity_sender.subscribe();
        // trade event
        tokio::spawn(async move {
            while let Ok(event) = trade_receiver.recv().await {
                trade_callback(event);
            }
        });
        // liquidity event
        tokio::spawn(async move {
            while let Ok(event) = liquidity_receiver.recv().await {
                liquidity_callback(event);
            }
        });
    }
    /// try connection
    async fn try_connection(&self) -> Result<(), String> {
        let _slot = self
            .client
            .get_slot()
            .await
            .map_err(|e| format!("Client Get Slot Error:{:?}", e));
        let _client = PubsubClient::new(&self.ws_url)
            .await
            .map_err(|e| format!("Client Connection Error:{:?}", e));
        Ok(())
    }
    /// listen pump.fun program
    async fn listen_to_program(
        ws_url: &str,
        program_id: &str,
        trade_sender: &broadcast::Sender<TradeEvent>,
        liquidity_sender: &broadcast::Sender<LiquidityEvent>,
    ) -> Result<(), String> {
        let client = PubsubClient::new(ws_url)
            .await
            .map_err(|e| format!("{:?}", e).to_string())?;
        let filter = RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()]);
        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::confirmed()),
        };
        let (mut notifications, unsubscribe) = client
            .logs_subscribe(filter, config)
            .await
            .map_err(|e| format!("{:?}", e).to_string())?;
        while let Some(result) = notifications.next().await {
            Self::handle_logs(&result, trade_sender, liquidity_sender).await;
        }
        let _ = unsubscribe().await;
        Ok(())
    }
    /// handle logs
    async fn handle_logs(
        response: &Response<solana_client::rpc_response::RpcLogsResponse>,
        trade_sender: &broadcast::Sender<TradeEvent>,
        liquidity_sender: &broadcast::Sender<LiquidityEvent>,
    ) {
        let notification = &response.value;
        let logs = &notification.logs;
        let signature = &notification.signature;
        if let Some(trade_event) = Self::handle_trade_event(&response, logs, signature).await {
            // send to channel
            let _ = trade_sender.send(trade_event);
        }
        if let Some(liquidity_event) =
            Self::handle_liquidity_event(&response, logs, signature).await
        {
            // send to channel
            let _ = liquidity_sender.send(liquidity_event);
        }
        // handle other events
        Self::handle_other_events(&response, logs, signature).await;
    }
    /// handle trade event
    async fn handle_trade_event(
        response: &Response<solana_client::rpc_response::RpcLogsResponse>,
        logs: &[String],
        signature: &str,
    ) -> Option<TradeEvent> {
        let _ = response;
        for log in logs {
            let lower_log = log.to_lowercase();
            let mut flag = false;
            let mut type_enum: Option<TradeLogInstructionType> =
                Some(TradeLogInstructionType::None);
            if lower_log.contains("trade") {
                flag = true;
                type_enum = Some(TradeLogInstructionType::Trade);
            }
            if lower_log.contains("swap") {
                flag = true;
                type_enum = Some(TradeLogInstructionType::Swap);
            }
            if lower_log.contains("buy") {
                flag = true;
                type_enum = Some(TradeLogInstructionType::Buy);
            }
            if lower_log.contains("sell") {
                flag = true;
                type_enum = Some(TradeLogInstructionType::Sell);
            }
            if flag {
                let mint = Self::get_mint_from_logs(logs).unwrap_or_else(|| "unknown".to_string());
                return Some(TradeEvent {
                    signature: signature.to_string(),
                    mint,
                    trade_event_type: type_enum.unwrap_or(TradeLogInstructionType::None),
                });
            }
        }
        None
    }
    /// handle liquidity event
    async fn handle_liquidity_event(
        response: &Response<solana_client::rpc_response::RpcLogsResponse>,
        logs: &[String],
        signature: &str,
    ) -> Option<LiquidityEvent> {
        let _ = response;
        for log in logs {
            let lower_log = log.to_lowercase();
            let mut flag = false;
            let mut type_enum: Option<LiquidityLogInstructionType> =
                Some(LiquidityLogInstructionType::None);

            if lower_log.contains("liquidity") {
                flag = true;
                type_enum = Some(LiquidityLogInstructionType::Liquidity);
            }
            if lower_log.contains("migrate") {
                flag = true;
                type_enum = Some(LiquidityLogInstructionType::Migrate);
            }
            if lower_log.contains("raydium") {
                flag = true;
                type_enum = Some(LiquidityLogInstructionType::Raydium);
            }
            if flag {
                let mint = Self::get_mint_from_logs(logs).unwrap_or_else(|| "unknown".to_string());
                return Some(LiquidityEvent {
                    signature: signature.to_string(),
                    mint,
                    liquidity_event_type: type_enum.unwrap_or(LiquidityLogInstructionType::None),
                });
            }
        }
        None
    }
    /// handle other event
    async fn handle_other_events(
        response: &Response<solana_client::rpc_response::RpcLogsResponse>,
        logs: &[String],
        signature: &str,
    ) {
        let _ = response;
        for log in logs {
            let lower_log = log.to_lowercase();
        }
    }
    /// get mint from logs.
    fn get_mint_from_logs(logs: &[String]) -> Option<String> {
        for log in logs {
            if log.len() >= 32 && log.len() <= 44 {
                if log.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Some(log.clone());
                }
            }
        }
        for log in logs {
            let words: Vec<&str> = log.split_whitespace().collect();
            for word in words {
                if word.len() >= 32
                    && word.len() <= 44
                    && word.chars().all(|c| c.is_ascii_alphanumeric())
                {
                    return Some(word.to_string());
                }
            }
        }
        None
    }
}
