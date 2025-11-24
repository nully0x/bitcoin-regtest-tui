use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use polar_core::{LightningImpl, NetworkStatus, NodeInfo};
use ratatui::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};

use crate::network_manager::NetworkManager;
use crate::ui;

/// Commands that can be sent to the app for async execution.
#[derive(Debug, Clone)]
pub enum AppCommand {
    CreateNetwork {
        name: String,
        lnd_count: usize,
        alias: String,
        lnd_version_idx: usize,
        btc_version_idx: usize,
    },
    StartNetwork,
    StopNetwork,
    DeleteNetwork,
    AddLightningNode {
        implementation: LightningImpl,
    },
    ViewNodeDetails,
    MineBlocks {
        num_blocks: u32,
    },
    FundWallet {
        node_name: String,
        amount: f64,
    },
    OpenChannel {
        from_node: String,
        to_node: String,
        capacity: u64,
        push_amount: Option<u64>,
    },
    SendPayment {
        from_node: String,
        to_node: String,
        amount: u64,
        memo: Option<String>,
    },
    SyncGraph,
    SyncChain,
}

/// UI mode - what screen we're showing
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    /// Create new network dialog
    CreateNetwork,
    /// Main application view
    #[default]
    Main,
    /// Node details view
    NodeDetails,
    /// Mine blocks dialog
    MineBlocks,
    /// Fund wallet dialog
    FundWallet,
    /// Open channel dialog
    OpenChannel,
    /// Send payment dialog
    SendPayment,
}

/// Active panel in the main UI
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    #[default]
    Networks,
    Nodes,
    Logs,
}

/// Application state
pub struct App {
    /// Is the application running
    pub running: bool,
    /// Current UI mode
    pub ui_mode: UiMode,
    /// Currently active panel
    pub active_panel: ActivePanel,
    /// Network manager
    pub network_manager: Arc<Mutex<NetworkManager>>,
    /// Cached network list
    pub networks: Vec<String>,
    /// Node names for selected network
    pub nodes: Vec<String>,
    /// Selected network index
    pub selected_network: Option<usize>,
    /// Selected node index
    pub selected_node: Option<usize>,
    /// Log scroll position
    pub log_scroll: usize,
    /// Cached log lines
    pub logs: Vec<String>,
    /// Status message
    pub status_message: Option<String>,
    /// Command sender for async operations
    command_tx: mpsc::UnboundedSender<AppCommand>,
    /// Command receiver for async operations
    command_rx: mpsc::UnboundedReceiver<AppCommand>,
    /// Network creation form state
    pub create_network_name: String,
    /// Number of LND nodes to create
    pub create_lnd_count: usize,
    /// Node alias prefix
    pub create_node_alias: String,
    /// Selected LND version index
    pub create_lnd_version_idx: usize,
    /// Selected Bitcoin version index
    pub create_btc_version_idx: usize,
    /// Active field in create network form (0=name, 1=alias, 2=lnd_count, 3=lnd_version, 4=btc_version)
    pub create_form_field: usize,
    /// Current node info being displayed
    pub node_info: Option<NodeInfo>,
    /// Node info scroll position
    pub node_info_scroll: usize,

    // Mine blocks form state
    /// Number of blocks to mine
    pub mine_blocks_count: String,

    // Fund wallet form state
    /// Selected node index for funding
    pub fund_node_idx: usize,
    /// Amount to fund (BTC)
    pub fund_amount: String,
    /// Active field in fund form (0=node, 1=amount)
    pub fund_form_field: usize,

    // Open channel form state
    /// From node index
    pub channel_from_idx: usize,
    /// To node index
    pub channel_to_idx: usize,
    /// Channel capacity (sats)
    pub channel_capacity: String,
    /// Push amount (sats)
    pub channel_push_amount: String,
    /// Active field in channel form (0=from, 1=to, 2=capacity, 3=push)
    pub channel_form_field: usize,

    // Send payment form state
    /// From node index
    pub payment_from_idx: usize,
    /// To node index
    pub payment_to_idx: usize,
    /// Payment amount (sats)
    pub payment_amount: String,
    /// Payment memo
    pub payment_memo: String,
    /// Active field in payment form (0=from, 1=to, 2=amount, 3=memo)
    pub payment_form_field: usize,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        let network_manager = NetworkManager::new().expect("Failed to create network manager");
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Self {
            running: true,
            ui_mode: UiMode::CreateNetwork, // Start with network creation
            active_panel: ActivePanel::default(),
            network_manager: Arc::new(Mutex::new(network_manager)),
            networks: Vec::new(),
            nodes: Vec::new(),
            selected_network: None,
            selected_node: None,
            log_scroll: 0,
            logs: Vec::new(),
            status_message: None,
            command_tx,
            command_rx,
            create_network_name: String::new(),
            create_lnd_count: 2, // Default to 2 LND nodes
            create_node_alias: String::new(),
            create_lnd_version_idx: 0, // Default to first version
            create_btc_version_idx: 0, // Default to first version
            create_form_field: 0,
            node_info: None,
            node_info_scroll: 0,
            // Lightning operation form defaults
            mine_blocks_count: "100".to_string(),
            fund_node_idx: 0,
            fund_amount: "1.0".to_string(),
            fund_form_field: 0,
            channel_from_idx: 0,
            channel_to_idx: 1,
            channel_capacity: "1000000".to_string(),
            channel_push_amount: "500000".to_string(),
            channel_form_field: 0,
            payment_from_idx: 0,
            payment_to_idx: 1,
            payment_amount: "10000".to_string(),
            payment_memo: String::new(),
            payment_form_field: 0,
        }
    }

    /// Initialize the app.
    pub async fn init(&mut self) -> Result<()> {
        // Check if Docker is available
        let manager = self.network_manager.lock().await;
        if let Err(e) = manager.check_docker().await {
            self.status_message = Some(format!("Docker not available: {}", e));
            self.ui_mode = UiMode::Main; // Skip to main even if Docker fails
        }
        drop(manager);

        // Load existing networks
        self.refresh_networks().await?;

        // If networks exist, start in Main view instead of CreateNetwork
        if !self.networks.is_empty() {
            self.ui_mode = UiMode::Main;
            self.selected_network = Some(0);
        }

        Ok(())
    }

    /// Refresh the cached network list.
    async fn refresh_networks(&mut self) -> Result<()> {
        let manager = self.network_manager.lock().await;
        self.networks = manager.networks().keys().cloned().collect();
        self.networks.sort();

        // Update nodes for selected network
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx) {
                if let Some(network) = manager.get_network(network_name) {
                    self.nodes = network
                        .nodes
                        .iter()
                        .map(|n| format!("{} ({})", n.name, n.kind))
                        .collect();
                }
            }
        }

        Ok(())
    }

    /// Run the main application loop
    ///
    /// # Errors
    ///
    /// Returns an error if drawing or event handling fails
    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while self.running {
            terminal.draw(|frame| ui::render(frame, self))?;
            self.handle_events()?;

            // Process any pending commands
            while let Ok(cmd) = self.command_rx.try_recv() {
                match cmd {
                    AppCommand::CreateNetwork {
                        name,
                        lnd_count,
                        alias,
                        lnd_version_idx,
                        btc_version_idx,
                    } => {
                        self.create_network(
                            name,
                            lnd_count,
                            alias,
                            lnd_version_idx,
                            btc_version_idx,
                        )
                        .await?;
                    }
                    AppCommand::StartNetwork => {
                        self.start_selected_network().await?;
                    }
                    AppCommand::StopNetwork => {
                        self.stop_selected_network().await?;
                    }
                    AppCommand::DeleteNetwork => {
                        self.delete_selected_network().await?;
                    }
                    AppCommand::AddLightningNode { implementation } => {
                        self.add_lightning_node(implementation).await?;
                    }
                    AppCommand::ViewNodeDetails => {
                        self.view_node_details().await?;
                    }
                    AppCommand::MineBlocks { num_blocks } => {
                        self.mine_blocks(num_blocks).await?;
                    }
                    AppCommand::FundWallet { node_name, amount } => {
                        self.fund_wallet(&node_name, amount).await?;
                    }
                    AppCommand::OpenChannel {
                        from_node,
                        to_node,
                        capacity,
                        push_amount,
                    } => {
                        self.open_channel(&from_node, &to_node, capacity, push_amount)
                            .await?;
                    }
                    AppCommand::SendPayment {
                        from_node,
                        to_node,
                        amount,
                        memo,
                    } => {
                        self.send_payment(&from_node, &to_node, amount, memo.as_deref())
                            .await?;
                    }
                    AppCommand::SyncGraph => {
                        self.sync_graph().await?;
                    }
                    AppCommand::SyncChain => {
                        self.sync_chain().await?;
                    }
                }
                // Redraw after processing command
                terminal.draw(|frame| ui::render(frame, self))?;
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key(key.code);
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        match self.ui_mode {
            UiMode::CreateNetwork => self.handle_create_network_key(code),
            UiMode::Main => self.handle_main_key(code),
            UiMode::NodeDetails => self.handle_node_details_key(code),
            UiMode::MineBlocks => self.handle_mine_blocks_key(code),
            UiMode::FundWallet => self.handle_fund_wallet_key(code),
            UiMode::OpenChannel => self.handle_open_channel_key(code),
            UiMode::SendPayment => self.handle_send_payment_key(code),
        }
    }

    fn handle_create_network_key(&mut self, code: KeyCode) {
        use polar_nodes::{BITCOIN_VERSIONS, LND_VERSIONS};

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Tab | KeyCode::Down => {
                self.create_form_field = (self.create_form_field + 1) % 5;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.create_form_field = if self.create_form_field == 0 {
                    4
                } else {
                    self.create_form_field - 1
                };
            }
            KeyCode::Char(c) => {
                match self.create_form_field {
                    0 => self.create_network_name.push(c), // Network name
                    1 => self.create_node_alias.push(c),   // Alias
                    _ => {}
                }
            }
            KeyCode::Backspace => match self.create_form_field {
                0 => {
                    self.create_network_name.pop();
                }
                1 => {
                    self.create_node_alias.pop();
                }
                _ => {}
            },
            KeyCode::Left => {
                match self.create_form_field {
                    2 => {
                        // LND count
                        if self.create_lnd_count > 1 {
                            self.create_lnd_count -= 1;
                        }
                    }
                    3 => {
                        // LND version
                        if self.create_lnd_version_idx > 0 {
                            self.create_lnd_version_idx -= 1;
                        }
                    }
                    4 => {
                        // Bitcoin version
                        if self.create_btc_version_idx > 0 {
                            self.create_btc_version_idx -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Right => {
                match self.create_form_field {
                    2 => {
                        // LND count
                        if self.create_lnd_count < 10 {
                            self.create_lnd_count += 1;
                        }
                    }
                    3 => {
                        // LND version
                        if self.create_lnd_version_idx < LND_VERSIONS.len() - 1 {
                            self.create_lnd_version_idx += 1;
                        }
                    }
                    4 => {
                        // Bitcoin version
                        if self.create_btc_version_idx < BITCOIN_VERSIONS.len() - 1 {
                            self.create_btc_version_idx += 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Enter => {
                // Create the network
                if !self.create_network_name.is_empty() {
                    let _ = self.command_tx.send(AppCommand::CreateNetwork {
                        name: self.create_network_name.clone(),
                        lnd_count: self.create_lnd_count,
                        alias: if self.create_node_alias.is_empty() {
                            self.create_network_name.clone() // Default to network name
                        } else {
                            self.create_node_alias.clone()
                        },
                        lnd_version_idx: self.create_lnd_version_idx,
                        btc_version_idx: self.create_btc_version_idx,
                    });
                }
            }
            _ => {}
        }
    }

    fn handle_main_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Tab => self.next_panel(),
            KeyCode::BackTab => self.prev_panel(),
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Char('n') => {
                // Open create network dialog
                self.ui_mode = UiMode::CreateNetwork;
                self.create_network_name.clear();
                self.create_node_alias.clear();
                self.create_lnd_count = 2;
                self.create_lnd_version_idx = 0;
                self.create_btc_version_idx = 0;
                self.create_form_field = 0;
            }
            KeyCode::Enter | KeyCode::Char('s') => {
                // Start network or view node details
                if self.active_panel == ActivePanel::Networks {
                    if self.selected_network.is_some() {
                        let _ = self.command_tx.send(AppCommand::StartNetwork);
                    }
                } else if self.active_panel == ActivePanel::Nodes {
                    // View node details
                    if self.selected_node.is_some() {
                        let _ = self.command_tx.send(AppCommand::ViewNodeDetails);
                    }
                }
            }
            KeyCode::Char('i') => {
                // View node info
                if self.active_panel == ActivePanel::Nodes && self.selected_node.is_some() {
                    let _ = self.command_tx.send(AppCommand::ViewNodeDetails);
                }
            }
            KeyCode::Char('x') => {
                // Stop network - send async command
                if self.active_panel == ActivePanel::Networks {
                    if self.selected_network.is_some() {
                        let _ = self.command_tx.send(AppCommand::StopNetwork);
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                // Delete network - send async command
                if self.active_panel == ActivePanel::Networks {
                    if self.selected_network.is_some() {
                        let _ = self.command_tx.send(AppCommand::DeleteNetwork);
                    }
                }
            }
            KeyCode::Char('a') => {
                // Add Lightning node to selected network
                if self.active_panel == ActivePanel::Networks {
                    if self.selected_network.is_some() {
                        // For now, default to LND. In the future, we can show a selection dialog
                        let _ = self.command_tx.send(AppCommand::AddLightningNode {
                            implementation: LightningImpl::Lnd,
                        });
                    }
                }
            }
            KeyCode::Char('m') => {
                // Mine blocks - only available when network is selected
                if self.selected_network.is_some() {
                    self.ui_mode = UiMode::MineBlocks;
                    self.mine_blocks_count = "100".to_string();
                }
            }
            KeyCode::Char('f') => {
                // Fund wallet - only available when nodes exist
                if self.selected_network.is_some() && !self.nodes.is_empty() {
                    self.ui_mode = UiMode::FundWallet;
                    self.fund_node_idx = 0;
                    self.fund_amount = "1.0".to_string();
                    self.fund_form_field = 0;
                }
            }
            KeyCode::Char('c') => {
                // Open channel - need at least 2 LND nodes
                if self.selected_network.is_some() && self.nodes.len() >= 2 {
                    self.ui_mode = UiMode::OpenChannel;
                    self.channel_from_idx = 0;
                    self.channel_to_idx = 1;
                    self.channel_capacity = "1000000".to_string();
                    self.channel_push_amount = "500000".to_string();
                    self.channel_form_field = 0;
                }
            }
            KeyCode::Char('p') => {
                // Send payment - need at least 2 LND nodes
                if self.selected_network.is_some() && self.nodes.len() >= 2 {
                    self.ui_mode = UiMode::SendPayment;
                    self.payment_from_idx = 0;
                    self.payment_to_idx = 1;
                    self.payment_amount = "10000".to_string();
                    self.payment_memo.clear();
                    self.payment_form_field = 0;
                }
            }
            KeyCode::Char('g') => {
                // Sync graph - synchronize LND nodes with each other
                if self.selected_network.is_some() {
                    let _ = self.command_tx.send(AppCommand::SyncGraph);
                }
            }
            KeyCode::Char('y') => {
                // Sync chain - ensure LND nodes are synced with Bitcoin blockchain
                if self.selected_network.is_some() {
                    let _ = self.command_tx.send(AppCommand::SyncChain);
                }
            }
            _ => {}
        }
    }

    fn handle_node_details_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Return to main view
                self.ui_mode = UiMode::Main;
                self.node_info = None;
                self.node_info_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.node_info_scroll = self.node_info_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.node_info_scroll = self.node_info_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    fn handle_mine_blocks_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.ui_mode = UiMode::Main;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                self.mine_blocks_count.push(c);
            }
            KeyCode::Backspace => {
                self.mine_blocks_count.pop();
            }
            KeyCode::Enter => {
                if let Ok(num_blocks) = self.mine_blocks_count.parse::<u32>() {
                    let _ = self.command_tx.send(AppCommand::MineBlocks { num_blocks });
                    self.ui_mode = UiMode::Main;
                }
            }
            _ => {}
        }
    }

    fn handle_fund_wallet_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.ui_mode = UiMode::Main;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.fund_form_field = (self.fund_form_field + 1) % 2;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.fund_form_field = if self.fund_form_field == 0 { 1 } else { 0 };
            }
            KeyCode::Left => {
                if self.fund_form_field == 0 && self.fund_node_idx > 0 {
                    self.fund_node_idx -= 1;
                }
            }
            KeyCode::Right => {
                if self.fund_form_field == 0
                    && self.fund_node_idx < self.nodes.len().saturating_sub(1)
                {
                    self.fund_node_idx += 1;
                }
            }
            KeyCode::Char(c) if self.fund_form_field == 1 && (c.is_ascii_digit() || c == '.') => {
                self.fund_amount.push(c);
            }
            KeyCode::Backspace if self.fund_form_field == 1 => {
                self.fund_amount.pop();
            }
            KeyCode::Enter => {
                if let Ok(amount) = self.fund_amount.parse::<f64>() {
                    if let Some(node_name) = self.nodes.get(self.fund_node_idx).cloned() {
                        let _ = self
                            .command_tx
                            .send(AppCommand::FundWallet { node_name, amount });
                        self.ui_mode = UiMode::Main;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_open_channel_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.ui_mode = UiMode::Main;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.channel_form_field = (self.channel_form_field + 1) % 4;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.channel_form_field = if self.channel_form_field == 0 {
                    3
                } else {
                    self.channel_form_field - 1
                };
            }
            KeyCode::Left => match self.channel_form_field {
                0 if self.channel_from_idx > 0 => self.channel_from_idx -= 1,
                1 if self.channel_to_idx > 0 => self.channel_to_idx -= 1,
                _ => {}
            },
            KeyCode::Right => match self.channel_form_field {
                0 if self.channel_from_idx < self.nodes.len().saturating_sub(1) => {
                    self.channel_from_idx += 1
                }
                1 if self.channel_to_idx < self.nodes.len().saturating_sub(1) => {
                    self.channel_to_idx += 1
                }
                _ => {}
            },
            KeyCode::Char(c) if c.is_ascii_digit() => match self.channel_form_field {
                2 => self.channel_capacity.push(c),
                3 => self.channel_push_amount.push(c),
                _ => {}
            },
            KeyCode::Backspace => match self.channel_form_field {
                2 => {
                    self.channel_capacity.pop();
                }
                3 => {
                    self.channel_push_amount.pop();
                }
                _ => {}
            },
            KeyCode::Enter => {
                if let (Ok(capacity), Ok(push)) = (
                    self.channel_capacity.parse::<u64>(),
                    self.channel_push_amount.parse::<u64>(),
                ) {
                    if let (Some(from), Some(to)) = (
                        self.nodes.get(self.channel_from_idx).cloned(),
                        self.nodes.get(self.channel_to_idx).cloned(),
                    ) {
                        let push_amount = if push > 0 { Some(push) } else { None };
                        let _ = self.command_tx.send(AppCommand::OpenChannel {
                            from_node: from,
                            to_node: to,
                            capacity,
                            push_amount,
                        });
                        self.ui_mode = UiMode::Main;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_send_payment_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.ui_mode = UiMode::Main;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.payment_form_field = (self.payment_form_field + 1) % 4;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.payment_form_field = if self.payment_form_field == 0 {
                    3
                } else {
                    self.payment_form_field - 1
                };
            }
            KeyCode::Left => match self.payment_form_field {
                0 if self.payment_from_idx > 0 => self.payment_from_idx -= 1,
                1 if self.payment_to_idx > 0 => self.payment_to_idx -= 1,
                _ => {}
            },
            KeyCode::Right => match self.payment_form_field {
                0 if self.payment_from_idx < self.nodes.len().saturating_sub(1) => {
                    self.payment_from_idx += 1
                }
                1 if self.payment_to_idx < self.nodes.len().saturating_sub(1) => {
                    self.payment_to_idx += 1
                }
                _ => {}
            },
            KeyCode::Char(c) => match self.payment_form_field {
                2 if c.is_ascii_digit() => self.payment_amount.push(c),
                3 => self.payment_memo.push(c),
                _ => {}
            },
            KeyCode::Backspace => match self.payment_form_field {
                2 => {
                    self.payment_amount.pop();
                }
                3 => {
                    self.payment_memo.pop();
                }
                _ => {}
            },
            KeyCode::Enter => {
                if let Ok(amount) = self.payment_amount.parse::<u64>() {
                    if let (Some(from), Some(to)) = (
                        self.nodes.get(self.payment_from_idx).cloned(),
                        self.nodes.get(self.payment_to_idx).cloned(),
                    ) {
                        let memo = if self.payment_memo.is_empty() {
                            None
                        } else {
                            Some(self.payment_memo.clone())
                        };
                        let _ = self.command_tx.send(AppCommand::SendPayment {
                            from_node: from,
                            to_node: to,
                            amount,
                            memo,
                        });
                        self.ui_mode = UiMode::Main;
                    }
                }
            }
            _ => {}
        }
    }

    fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Networks => ActivePanel::Nodes,
            ActivePanel::Nodes => ActivePanel::Logs,
            ActivePanel::Logs => ActivePanel::Networks,
        };
    }

    fn prev_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Networks => ActivePanel::Logs,
            ActivePanel::Nodes => ActivePanel::Networks,
            ActivePanel::Logs => ActivePanel::Nodes,
        };
    }

    fn select_prev(&mut self) {
        match self.active_panel {
            ActivePanel::Networks => {
                if let Some(idx) = self.selected_network {
                    self.selected_network = Some(idx.saturating_sub(1));
                }
            }
            ActivePanel::Nodes => {
                if let Some(idx) = self.selected_node {
                    self.selected_node = Some(idx.saturating_sub(1));
                }
            }
            ActivePanel::Logs => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
        }
    }

    fn select_next(&mut self) {
        match self.active_panel {
            ActivePanel::Networks => {
                let max = self.networks.len().saturating_sub(1);
                self.selected_network = Some(
                    self.selected_network
                        .map_or(0, |i| i.saturating_add(1).min(max)),
                );
            }
            ActivePanel::Nodes => {
                let max = self.nodes.len().saturating_sub(1);
                self.selected_node = Some(
                    self.selected_node
                        .map_or(0, |i| i.saturating_add(1).min(max)),
                );
            }
            ActivePanel::Logs => {
                self.log_scroll = self.log_scroll.saturating_add(1);
            }
        }
    }

    /// Create a new network.
    pub async fn create_network(
        &mut self,
        name: String,
        lnd_count: usize,
        alias: String,
        lnd_version_idx: usize,
        btc_version_idx: usize,
    ) -> Result<()> {
        use polar_nodes::{BITCOIN_VERSIONS, LND_VERSIONS};

        self.status_message = Some(format!("Creating network '{}'...", name));

        let lnd_version = LND_VERSIONS
            .get(lnd_version_idx)
            .unwrap_or(&polar_nodes::LndNode::DEFAULT_IMAGE);
        let btc_version = BITCOIN_VERSIONS
            .get(btc_version_idx)
            .unwrap_or(&polar_nodes::BitcoinNode::DEFAULT_IMAGE);

        let mut manager = self.network_manager.lock().await;
        match manager.create_network_with_config(&name, lnd_count, &alias, lnd_version, btc_version)
        {
            Ok(_) => {
                self.status_message = Some(format!("Network '{}' created successfully", name));
                self.ui_mode = UiMode::Main;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to create network: {}", e));
            }
        }
        drop(manager);

        self.refresh_networks().await?;
        if !self.networks.is_empty() {
            self.selected_network = Some(0);
        }

        Ok(())
    }

    /// Start the selected network.
    pub async fn start_selected_network(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some(format!("Starting network '{}'...", network_name));

                let mut manager = self.network_manager.lock().await;
                match manager.start_network(&network_name).await {
                    Ok(_) => {
                        self.status_message =
                            Some(format!("Network '{}' started successfully", network_name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to start network: {}", e));
                    }
                }
                drop(manager);

                self.refresh_networks().await?;
            }
        }
        Ok(())
    }

    /// Stop the selected network.
    pub async fn stop_selected_network(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some(format!("Stopping network '{}'...", network_name));

                let mut manager = self.network_manager.lock().await;
                match manager.stop_network(&network_name).await {
                    Ok(_) => {
                        self.status_message =
                            Some(format!("Network '{}' stopped successfully", network_name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to stop network: {}", e));
                    }
                }
                drop(manager);

                self.refresh_networks().await?;
            }
        }
        Ok(())
    }

    /// Get the status of the selected network.
    pub async fn get_selected_network_status(&self) -> Option<NetworkStatus> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx) {
                let manager = self.network_manager.lock().await;
                return manager.get_network(network_name).map(|n| n.status);
            }
        }
        None
    }

    /// Delete the selected network.
    pub async fn delete_selected_network(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some(format!("Deleting network '{}'...", network_name));

                let mut manager = self.network_manager.lock().await;
                match manager.delete_network(&network_name).await {
                    Ok(_) => {
                        self.status_message =
                            Some(format!("Network '{}' deleted successfully", network_name));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to delete network: {}", e));
                    }
                }
                drop(manager);

                self.refresh_networks().await?;

                // Adjust selection after deletion
                if self.networks.is_empty() {
                    self.selected_network = None;
                    self.nodes.clear();
                    self.selected_node = None;
                } else if idx >= self.networks.len() {
                    self.selected_network = Some(self.networks.len().saturating_sub(1));
                }
            }
        }
        Ok(())
    }

    /// Add a Lightning node to the selected network.
    pub async fn add_lightning_node(&mut self, implementation: LightningImpl) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some(format!(
                    "Adding {} node to '{}'...",
                    implementation, network_name
                ));

                let mut manager = self.network_manager.lock().await;
                match manager
                    .add_lightning_node(&network_name, implementation)
                    .await
                {
                    Ok(node_name) => {
                        self.status_message = Some(format!(
                            "{} node '{}' added successfully",
                            implementation, node_name
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to add node: {}", e));
                    }
                }
                drop(manager);

                self.refresh_networks().await?;
            }
        }
        Ok(())
    }

    /// View details for the selected node.
    pub async fn view_node_details(&mut self) -> Result<()> {
        if let Some(network_idx) = self.selected_network {
            if let Some(node_idx) = self.selected_node {
                if let Some(network_name) = self.networks.get(network_idx) {
                    let manager = self.network_manager.lock().await;

                    // Get the node name from the cached nodes list
                    if let Some(node_display) = self.nodes.get(node_idx) {
                        // Parse the node name from "name (type)" format
                        let node_name = node_display.split(" (").next().unwrap_or("").to_string();

                        match manager.get_node_info(network_name, &node_name).await {
                            Ok(info) => {
                                self.node_info = Some(info);
                                self.node_info_scroll = 0;
                                self.ui_mode = UiMode::NodeDetails;
                                self.status_message = None;
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(format!("Failed to get node info: {}", e));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn mine_blocks(&mut self, num_blocks: u32) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some(format!("Mining {} blocks...", num_blocks));

                let manager = self.network_manager.lock().await;

                match manager.mine_blocks(&network_name, num_blocks).await {
                    Ok(hashes) => {
                        self.status_message =
                            Some(format!("Mined {} blocks successfully", hashes.len()));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to mine blocks: {}", e));
                    }
                }
            } else {
                self.status_message = Some("No network selected".to_string());
            }
        } else {
            self.status_message = Some("No network selected".to_string());
        }
        Ok(())
    }

    pub async fn fund_wallet(&mut self, node_name: &str, amount: f64) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                // Parse node name from "name (type)" format if needed
                let actual_node_name = node_name.split(" (").next().unwrap_or(node_name);

                self.status_message = Some(format!(
                    "Funding {} with {} BTC...",
                    actual_node_name, amount
                ));

                let manager = self.network_manager.lock().await;
                match manager
                    .fund_lnd_wallet(&network_name, actual_node_name, amount)
                    .await
                {
                    Ok(txid) => {
                        self.status_message = Some(format!("Funded wallet. TXID: {}", &txid[..8]));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to fund wallet: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn open_channel(
        &mut self,
        from: &str,
        to: &str,
        capacity: u64,
        push_amount: Option<u64>,
    ) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                // Parse node names from "name (type)" format if needed
                let actual_from = from.split(" (").next().unwrap_or(from);
                let actual_to = to.split(" (").next().unwrap_or(to);

                let push_desc = if let Some(p) = push_amount {
                    format!(" (push {})", p)
                } else {
                    String::new()
                };
                self.status_message = Some(format!(
                    "Opening channel {} → {} capacity: {}{}",
                    actual_from, actual_to, capacity, push_desc
                ));

                let manager = self.network_manager.lock().await;
                match manager
                    .open_channel(&network_name, actual_from, actual_to, capacity, push_amount)
                    .await
                {
                    Ok(txid) => {
                        self.status_message =
                            Some(format!("Channel opened. Funding TXID: {}", &txid[..8]));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to open channel: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_payment(
        &mut self,
        from: &str,
        to: &str,
        amount: u64,
        memo: Option<&str>,
    ) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                // Parse node names from "name (type)" format if needed
                let actual_from = from.split(" (").next().unwrap_or(from);
                let actual_to = to.split(" (").next().unwrap_or(to);

                let memo_desc = memo.map(|m| format!(" '{}'", m)).unwrap_or_default();
                self.status_message = Some(format!(
                    "Sending {} sats from {} → {}{}",
                    amount, actual_from, actual_to, memo_desc
                ));

                let manager = self.network_manager.lock().await;
                match manager
                    .send_payment(&network_name, actual_from, actual_to, amount, memo)
                    .await
                {
                    Ok(payment_hash) => {
                        self.status_message =
                            Some(format!("Payment sent! Hash: {}", &payment_hash[..16]));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to send payment: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn sync_graph(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some("Syncing Lightning Network graph...".to_string());

                let manager = self.network_manager.lock().await;
                match manager.sync_graph(&network_name).await {
                    Ok(synced_nodes) => {
                        self.status_message = Some(format!(
                            "Graph synced! {} LND nodes synchronized",
                            synced_nodes
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to sync graph: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn sync_chain(&mut self) -> Result<()> {
        if let Some(idx) = self.selected_network {
            if let Some(network_name) = self.networks.get(idx).cloned() {
                self.status_message = Some("Syncing LND nodes with blockchain...".to_string());

                let manager = self.network_manager.lock().await;
                match manager.sync_chain(&network_name).await {
                    Ok(synced_nodes) => {
                        self.status_message = Some(format!(
                            "Chain synced! {} LND nodes synchronized with blockchain",
                            synced_nodes
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to sync chain: {}", e));
                    }
                }
            }
        }
        Ok(())
    }
}
