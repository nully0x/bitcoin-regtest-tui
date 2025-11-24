//! Main layout rendering for the TUI.

use polar_core::{BitcoinNodeInfo, LndNodeInfo, NodeInfo};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::{ActivePanel, App, UiMode};

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &App) {
    match app.ui_mode {
        UiMode::CreateNetwork => render_create_network(frame, app),
        UiMode::Main => render_main(frame, app),
        UiMode::NodeDetails => render_node_details(frame, app),
    }
}

/// Render the main application view.
fn render_main(frame: &mut Frame, app: &App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(35),
            Constraint::Percentage(40),
        ])
        .split(main_chunks[0]);

    render_networks_panel(frame, app, chunks[0]);
    render_nodes_panel(frame, app, chunks[1]);
    render_logs_panel(frame, app, chunks[2]);
    render_status_bar(frame, app, main_chunks[1]);
}

/// Render the create network dialog.
fn render_create_network(frame: &mut Frame, app: &App) {
    use polar_nodes::{BITCOIN_VERSIONS, LND_VERSIONS};

    // Center the dialog
    let area = centered_rect(85, 70, frame.area());

    // Clear the background
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        frame.area(),
    );

    // Create the dialog content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Network name
            Constraint::Length(3), // Alias
            Constraint::Length(3), // LND count
            Constraint::Length(3), // LND version
            Constraint::Length(3), // Bitcoin version
            Constraint::Min(1),    // Help text
        ])
        .split(area);

    // Dialog box
    let block = Block::default()
        .title(" Create Network ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    // Title
    let title = Paragraph::new("Configure your Lightning Network").style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(title, chunks[0]);

    // Helper function for field style
    let field_style = |field_idx: usize| {
        if app.create_form_field == field_idx {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Network name field (field 0)
    let name_text = if app.create_network_name.is_empty() {
        Line::from(vec![
            Span::styled("Network Name: ", field_style(0)),
            Span::styled("_", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Network Name: ", field_style(0)),
            Span::styled(&app.create_network_name, field_style(0)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ])
    };
    frame.render_widget(Paragraph::new(name_text), chunks[1]);

    // Alias field (field 1)
    let alias_text = if app.create_node_alias.is_empty() {
        Line::from(vec![
            Span::styled("Node Alias Prefix: ", field_style(1)),
            Span::styled(
                "(defaults to network name)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("Node Alias Prefix: ", field_style(1)),
            Span::styled(&app.create_node_alias, field_style(1)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ])
    };
    frame.render_widget(Paragraph::new(alias_text), chunks[2]);

    // LND count field (field 2)
    let count_text = Line::from(vec![
        Span::styled("LND Nodes: ", field_style(2)),
        Span::styled("< ", Style::default().fg(Color::DarkGray)),
        Span::styled(app.create_lnd_count.to_string(), field_style(2)),
        Span::styled(" >", Style::default().fg(Color::DarkGray)),
        Span::styled("  (use ←/→)", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(count_text), chunks[3]);

    // LND version field (field 3)
    let lnd_ver = LND_VERSIONS
        .get(app.create_lnd_version_idx)
        .unwrap_or(&"unknown");
    let lnd_ver_short = lnd_ver.split(':').last().unwrap_or(lnd_ver);
    let lnd_version_text = Line::from(vec![
        Span::styled("LND Version: ", field_style(3)),
        Span::styled("< ", Style::default().fg(Color::DarkGray)),
        Span::styled(lnd_ver_short, field_style(3)),
        Span::styled(" >", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "  ({}/{})",
                app.create_lnd_version_idx + 1,
                LND_VERSIONS.len()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(lnd_version_text), chunks[4]);

    // Bitcoin version field (field 4)
    let btc_ver = BITCOIN_VERSIONS
        .get(app.create_btc_version_idx)
        .unwrap_or(&"unknown");
    let btc_ver_short = btc_ver.split(':').last().unwrap_or(btc_ver);
    let btc_version_text = Line::from(vec![
        Span::styled("Bitcoin Version: ", field_style(4)),
        Span::styled("< ", Style::default().fg(Color::DarkGray)),
        Span::styled(btc_ver_short, field_style(4)),
        Span::styled(" >", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "  ({}/{})",
                app.create_btc_version_idx + 1,
                BITCOIN_VERSIONS.len()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(btc_version_text), chunks[5]);

    // Help text
    let help = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab/↑↓", Style::default().fg(Color::Cyan)),
            Span::raw(": Switch field  |  "),
            Span::styled("←/→", Style::default().fg(Color::Cyan)),
            Span::raw(": Adjust value  |  "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(": Create  |  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "All LND nodes will connect to 1 Bitcoin Core node in regtest mode",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];
    frame.render_widget(Paragraph::new(help), chunks[6]);
}

/// Render the networks panel (left).
fn render_networks_panel(frame: &mut Frame, app: &App, area: Rect) {
    let style = panel_style(app.active_panel == ActivePanel::Networks);

    let items: Vec<ListItem> = app
        .networks
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let content = if Some(i) == app.selected_network {
                Line::from(vec![
                    Span::raw("> "),
                    Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(format!("  {name}"))
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Networks ")
            .borders(Borders::ALL)
            .border_style(style),
    );

    frame.render_widget(list, area);
}

/// Render the nodes panel (center).
fn render_nodes_panel(frame: &mut Frame, app: &App, area: Rect) {
    let style = panel_style(app.active_panel == ActivePanel::Nodes);

    let items: Vec<ListItem> = app
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let content = if Some(i) == app.selected_node {
                Line::from(vec![
                    Span::raw("> "),
                    Span::styled(node, Style::default().add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(format!("  {node}"))
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Nodes ")
            .borders(Borders::ALL)
            .border_style(style),
    );

    frame.render_widget(list, area);
}

/// Render the logs panel (right).
fn render_logs_panel(frame: &mut Frame, app: &App, area: Rect) {
    let style = panel_style(app.active_panel == ActivePanel::Logs);

    let text: Vec<Line> = app.logs.iter().map(|l| Line::from(l.as_str())).collect();

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Logs ")
                .borders(Borders::ALL)
                .border_style(style),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Render the status bar (bottom).
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = vec![Line::from(vec![
        Span::raw("Tab: Switch Panel | "),
        Span::raw("↑↓/k/j: Navigate | "),
        Span::styled("n", Style::default().fg(Color::Cyan)),
        Span::raw(": New | "),
        Span::styled("s", Style::default().fg(Color::Green)),
        Span::raw(": Start | "),
        Span::styled("x", Style::default().fg(Color::Red)),
        Span::raw(": Stop | "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(": Add Node | "),
        Span::styled("d", Style::default().fg(Color::Red)),
        Span::raw(": Delete | "),
        Span::styled("i/Enter", Style::default().fg(Color::Magenta)),
        Span::raw(": Info | "),
        Span::raw("q: Quit"),
    ])];

    let mut status_lines = help_text;

    if let Some(ref msg) = app.status_message {
        status_lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Cyan)),
            Span::raw(msg),
        ]));
    }

    let status = Paragraph::new(status_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    );

    frame.render_widget(status, area);
}

/// Get border style based on whether panel is active.
fn panel_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Helper to create a centered rect.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Render the node details view.
fn render_node_details(frame: &mut Frame, app: &App) {
    let area = centered_rect(90, 85, frame.area());

    // Clear the background
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        frame.area(),
    );

    if let Some(ref node_info) = app.node_info {
        let mut lines = Vec::new();

        match node_info {
            NodeInfo::Bitcoin(info) => {
                lines.extend(render_bitcoin_info(info));
            }
            NodeInfo::Lnd(info) => {
                lines.extend(render_lnd_info(info));
            }
        }

        // Add help text at the bottom
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("↑↓/j/k", Style::default().fg(Color::Cyan)),
            Span::raw(": Scroll  |  "),
            Span::styled("Esc/q", Style::default().fg(Color::Red)),
            Span::raw(": Back"),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Node Details ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.node_info_scroll as u16, 0));

        frame.render_widget(paragraph, area);
    } else {
        let text = Paragraph::new("No node information available").block(
            Block::default()
                .title(" Node Details ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        );
        frame.render_widget(text, area);
    }
}

/// Render Bitcoin Core node information.
fn render_bitcoin_info(info: &BitcoinNodeInfo) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            "Bitcoin Core Node",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Version:        ", Style::default().fg(Color::Cyan)),
            Span::raw(info.version.clone()),
        ]),
        Line::from(vec![
            Span::styled("Chain:          ", Style::default().fg(Color::Cyan)),
            Span::raw(info.chain.clone()),
        ]),
        Line::from(vec![
            Span::styled("Block Height:   ", Style::default().fg(Color::Cyan)),
            Span::raw(info.blocks.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Connections:    ", Style::default().fg(Color::Cyan)),
            Span::raw(info.connections.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Difficulty:     ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:.8}", info.difficulty)),
        ]),
        Line::from(vec![
            Span::styled("IBD Complete:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if info.ibd_complete { "Yes" } else { "No" },
                if info.ibd_complete {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Network Endpoints",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("RPC:            ", Style::default().fg(Color::Cyan)),
            Span::raw(info.rpc_host.clone()),
        ]),
        Line::from(vec![
            Span::styled("P2P:            ", Style::default().fg(Color::Cyan)),
            Span::raw(info.p2p_host.clone()),
        ]),
    ]
}

/// Render LND node information.
fn render_lnd_info(info: &LndNodeInfo) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            "LND Node",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Alias:          ", Style::default().fg(Color::Cyan)),
            Span::raw(info.alias.clone()),
        ]),
        Line::from(vec![
            Span::styled("Version:        ", Style::default().fg(Color::Cyan)),
            Span::raw(info.version.clone()),
        ]),
        Line::from(vec![
            Span::styled("Identity:       ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}...", &info.identity_pubkey[..20])),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sync Status",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Chain Synced:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if info.synced_to_chain { "Yes" } else { "No" },
                if info.synced_to_chain {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Graph Synced:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if info.synced_to_graph { "Yes" } else { "No" },
                if info.synced_to_graph {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Block Height:   ", Style::default().fg(Color::Cyan)),
            Span::raw(info.block_height.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Block Hash:     ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}...", &info.block_hash[..20])),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Network",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Peers:          ", Style::default().fg(Color::Cyan)),
            Span::raw(info.num_peers.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Active Channels:", Style::default().fg(Color::Cyan)),
            Span::raw(info.num_active_channels.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Pending Channels:", Style::default().fg(Color::Cyan)),
            Span::raw(info.num_pending_channels.to_string()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Balances",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Wallet Balance: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{} sats", info.wallet_balance),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Channel Balance:", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{} sats", info.channel_balance),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Endpoints",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("REST API:       ", Style::default().fg(Color::Cyan)),
            Span::raw(info.rest_host.clone()),
        ]),
        Line::from(vec![
            Span::styled("gRPC:           ", Style::default().fg(Color::Cyan)),
            Span::raw(info.grpc_host.clone()),
        ]),
    ]
}
